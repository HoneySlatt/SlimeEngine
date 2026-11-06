# 0186. Counters and ratios in CI

- Status: Accepted; amends [0005](0005-lints-and-unsafe-policy.md) (`unsafe` allowed in a
  measuring tool that the engine does not ship), [0104](0104-the-build-ci-and-measurements.md)
  (the benchmarks run in CI, their counters and ratios checked, their times not) and
  [0175](0175-the-quality-bar.md) (M6c.Q2b cut in two; the ratio of the editor's interface);
  amended by [0189](0189-first-measures-of-time.md) (M6c.Q2d cut in two, memory and power in
  M6c.Q2e)
- Date: 2026-11-05

## Context

[ADR-0175](0175-the-quality-bar.md) decided that CI checks budgets through exact counters and
ratios of times, figures that shared runners and lavapipe keep steady, and that absolute times are
measured by hand. Until this milestone, CI checked none:

- The benchmarks of [`slime-benches`](../../benches/README.md) computed counters, draw calls,
  instances drawn, items, glyphs, elements, the bytes of a save, but printed them as text beside
  their times, and [ADR-0104](0104-the-build-ci-and-measurements.md) kept the benchmarks out of CI.
- Nothing measured the editor's interface whole: `editor.hierarchy.100k` measured one panel.
- Nothing counted what a frame of the interface visits, nor what it allocates.

Measured on 2026-11-05 on the owner's machine, a Ryzen 7 5800X:

- In a debug build on lavapipe, as CI builds and draws, the benchmarks with counters took some 70 s
  for `render.instances.10k` alone, 30 frames at each of three sizes, and 33 s for
  `ui.tree.editor`. Once each series is cut to one run, every counter takes about 65 s on 4 cores.
- A counter is the same at every run, in a debug and in a release build.

The owner decided on 2026-11-05, one question at a time: the milestone's cut, where the counters
live, how the benchmarks open the zone, how allocations are counted, and the ratio's figure.

## Decision

**The milestone cut in two.** M6c.Q2b keeps the counters and ratios in CI, the benchmark of the
editor's whole interface and the CPU benchmarks on 4 cores. A new milestone, M6c.Q2d, after
M6c.Q2c, takes the first measures of a start, of input to present, of memory and of power, the
variable refresh rates, `hyperfine` and `heaptrack`, and the figures proposed for the budgets that
have none: input to present and variable refresh rates depend on the frames' pacing, which M6c.Q2c
changes.

**The benchmarks count in CI.** A row of a benchmark is a time, a counter or a ratio
(`measure::Check`). `slime-benches --counters` runs the benchmarks that give counters, each series
cut to one run (`Context::runs`), except a ratio's, and compares their counters with those that
[`benches/counters.txt`](../../benches/counters.txt) records: any difference fails, a rise as a
fall, a counter new or gone. A change that changes a counter on purpose writes the file again in
the same commit (`--write-counters`), which a review reads. A ratio fails above its budget. Times
are printed and never checked. `scripts/check.sh game` runs it in a debug build on lavapipe, the
zone's benchmark apart, since only that part has the zone cooked.

**Counters of the interface.** `uwui`'s `FrameStats::visited` counts the elements that a frame's
walks reach, which a frame costs even when nothing changed. The benchmarks of the interface count,
for a frame standing still: its elements, those visited, the items of its display list, and the
calling thread's allocations.

**Allocations counted by the benchmarks' allocator.** `slime-benches` installs the system's
allocator wrapped to count, per thread, the calls that take memory, so that a benchmark counts
what it measures and not what the driver's threads allocate. Its 20 lines are the repository's
first `unsafe` code: `GlobalAlloc` is an `unsafe` trait. ADR-0005 allowed `unsafe` only to call
the system or for a measured speed; it now allows it also in a measuring tool that the engine and
its games do not ship, on the smallest item, with its `SAFETY` comments. It does not run under
Miri: a nightly toolchain in CI for 20 lines that hand every call to the system's allocator.

**The editor's whole interface, measured.** `slime_editor::Offscreen` runs the editor's window
without a window of the system, frame by frame, as the program does: the core answers what the
window asked and runs the fixed steps, the window builds, styles, lays out and gathers its
interface, then draws it through the canvas. `window_image` now draws through it. `editor.window.zone` measures it on *Hunna Wood*'s
zone, `editor.window.entities` on the examples' yard with 10 000 and 100 000 entities more, at the
high tier's screen, 3840 × 2160: a frame standing still, one after the selection changed, one after
a step, whole and its interface's part. Their allocations are the interface's part's alone: the
core runs fixed steps by the clock, and the draw allocates in the GPU's driver, which vary from one
machine to another. The two levels' frames run in turn, so that what slows a shared runner slows
both series alike.

**The benchmarks open the zone through its editor's library.** `hunnawood-editor` is a library and
a program: its library gives `program()`, the editor's program on the zone, which its `main` runs
and the benchmarks open. `slime-benches` depends on it, the only game package it depends on.

**The ratio of the editor's interface.** From a level of 10 000 entities to one of 100 000, the
CPU of each kind of frame's interface may grow at most 1.5 times
([`ROADMAP.md`](../ROADMAP.md#4-quality-bar)). The core's part and the canvas's draw are left out
of the ratio: the first runs the fixed steps over the whole world, as a game's does, and the second
does not grow with the level, which the items counted check, and its time on lavapipe varies most.

## Alternatives considered

- **A test in each crate for its counters**: `cargo test` alone, the counters beside their code;
  but every scenario written twice, as a benchmark and as a test, the two drifting apart, and the
  figures scattered over ten tests.
- **A bound above each counter**, 25 % over its value: fewer writes of the file as the interface
  changes, but a rise of 20 % passes unseen, and a fall is never recorded.
- **The benchmarks run in a release build in CI**: series as long as by hand, but a release build
  of the workspace in a Linux job, minutes that the limit of 20 minutes does not have.
- **The editor's interface measured on the yard alone**: no new dependency, but not the zone that
  the milestone names, its prefabs, its names and its textures.
- **A hidden option of the editor's programs that measures**: any game's editor measurable, but
  measuring code in a program that users run, and an option that they see.
- **`allocation-counter`, a crate**: no `unsafe` in the repository, but the same `unsafe` in a
  crate without a release since 2023, for 20 lines.
- **Allocations measured by hand with `heaptrack`**: nothing new, but a regression unseen until
  someone measures.
- **A ratio of 1.2 or of 2**: the first fails at random on shared runners, whose series vary by 10
  to 20 %; the second lets a cost that doubles pass.

## Consequences

- `check (game)` runs the counters, about a minute on the owner's machine pinned to 4 cores, more
  on a runner: [`measurements.md`](../measurements.md) holds CI's times.
- A change of the interface, the renderer or the world that changes a counter writes
  `benches/counters.txt` again, and its review says why.
- The counters are checked on Linux only.
- A frame of the interface standing still allocates, which the counter holds without a budget.
- The editor's interface passes its budget in [ADR-0041](0041-render-graph.md)'s stall, which its
  canvas's draw meets on the owner's machine once other programs fill the memory that the CPU
  maps: debt.
- `ARCHITECTURE.md` names `slime-benches` as the crate that holds `unsafe` code.
