# 0175. The quality bar

- Status: Accepted; amends [0104](0104-the-build-ci-and-measurements.md) and
  [0114](0114-hardware-tiers-and-the-gpus-limits.md) (CI checks budgets through counters and ratios
  of times; the budgets' figures live in the roadmap; CPU benchmarks also run on 4 cores),
  [0155](0155-the-editors-design.md) (the quality milestones before M6c.5f and the stage's audit
  before M6c.9; an idle editor wakes itself not at all), and moves to the roadmap the figures of
  [0110](0110-how-the-editor-runs-a-games-code.md), [0123](0123-the-linux-checks-in-two-jobs.md),
  [0137](0137-the-canvas.md), [0138](0138-the-high-tier-at-3840-2160.md) and
  [0140](0140-the-tree-of-elements.md)
- Date: 2026-11-05

## Context

The engine's budgets were scattered, and many were never checked:

- Their figures sat in the ADRs that decided them: Play's time to control in ADR-0110, the tiers,
  their frames and their memory in ADR-0114, CI's 20 minutes in ADR-0123, the interface's GPU time
  in ADR-0137, the high tier's resolution in ADR-0138, which changed ADR-0114's, a list's frame in
  ADR-0140. Others sat in code: the sample zone's budgets in its test, the development channel's
  answer in its own.
- CI ran no benchmark ([ADR-0104](0104-the-build-ci-and-measurements.md),
  [ADR-0114](0114-hardware-tiers-and-the-gpus-limits.md)): every time was measured by hand, when a
  milestone thought of it.
- The editor's idle check allowed two frames in 10 seconds, and counted no wake-up.
- Nothing measured the start of a program, the time from an input to the image that shows it,
  memory over a long session, or power.
- Readers of files were tested with damaged copies, not fuzzed
  ([ADR-0102](0102-a-damaged-file-gives-an-error.md)); a crash left nothing for the
  user to report; no audit had covered a whole stage since the audit of 0.4.

The editor and the engine's interface aim at the most refined interfaces of the time, at
3840 × 2160 and 240 frames per second on the high tier: 4.17 ms a frame. The owner decided on
2026-11-05, one by one, the budgets below and how they are checked.

## Decision

**One home for the figures.** The *quality bar*, section 4 of [`ROADMAP.md`](../ROADMAP.md),
holds the references the engine is weighed against, the hardware tiers, and every budget: its
figure, what measures it, where it is checked, and whether it is met. ADRs say why a budget is what
it is; [`measurements.md`](../measurements.md) holds what was measured; [`debt.md`](../debt.md)
what is missed. The figures that earlier ADRs give stand as decided at their date; the roadmap's
are current. From this decision on, a test that checks a budget holds its figure, and names the
roadmap.

A budget is what a user or a player feels or pays: time, memory, power, a start, a program at rest,
CI's jobs. The limits of formats and the sizes of caches are contracts, which stay in rustdoc and
their ADRs.

**How budgets are checked.** CI checks budgets through exact counters, draws, elements, the bytes
of a save, allocations at rest, and ratios of times, how a cost grows with its size: figures that
shared runners and lavapipe keep steady. Absolute times and the GPU's are measured by hand on the
owner's machine, at each milestone that touches them, into `measurements.md`. A time may be
checked in CI only against a bound far above it, which shared runners keep, as the development
channel's answer is. A budget missed is debt until it is met.

**At rest.** The editor, its floating panels and applications draw no frame and wake themselves
not at all: zero frames and zero own wake-ups per second, an *own wake-up* being a thread of the
program leaving its wait when nothing outside asked. A caret that blinks stops after 10 seconds
without input; the caret does not blink today. A game paused or hidden draws nothing. CI checks it
in a headless Weston, from the process's context switches read in `/proc/<pid>/status` over 10
seconds; which threads count is the design of the milestone that builds it.

**Input to present.** At the 99th percentile, at most one frame for the editor and applications,
two for games, from the input's arrival to the image's present, measured inside the engine. The
compositor and the display add time that is stated as not measured.

**Start.** The editor and applications show their first useful frame within 150 ms, warm. A level
opened, a game to control, and cold starts are measured first; their figures are proposed after.

**The editor's interface.** At most 1.5 ms of CPU a frame on the high tier, at 240 frames per
second: about a third of the frame, the rest left to the viewports and the GPU's work. The
hierarchy's search, 5.43 ms at 100 000 entities, is spread over frames; debt until it is.

**Power and memory.** Two rules now: the editor at rest adds no power to the GPU's beyond the
desktop's; memory does not grow over 100 undos and redos and 10 runs of Play. The editor's memory
and video memory on the zone, and the power of a game capped at 60 frames per second, are measured
first; their figures are proposed after.

**The handheld tier's proxy.** Until a handheld is measured, CPU benchmarks also run pinned to 4
cores (`taskset`). Its GPU stays unmeasured. A Steam Deck is bought before Stage 9a.

**CI's jobs.** Each takes at most 20 minutes, its time recorded at each milestone in
`measurements.md`; over it is debt. The workflow's timeouts, 30 and 45 minutes, stay as a safety
net.

**Criteria without a figure**, each with its stage: games' accessibility, controls that can be
remapped, subtitles, modes for colour-blind players, camera shake and motion blur that can be
turned off, at Stage 7; HDR output and a wide colour gamut at Stage 9a; variable refresh rates
checked against [ADR-0021](0021-frame-pacing.md)'s pacing at M6c.Q2b, then stated supported or not.
Fuzzing and crash reports are [ADR-0176](0176-fuzzing-and-crash-reports.md)'s; audits and releases
[ADR-0177](0177-audits-and-releases.md)'s.

**Tools.** `hyperfine`, for starts, and `heaptrack`, for memory, enter the development shell,
`dependencies.md` and `CONTRIBUTING.md` with the milestone that first uses them.

**Milestones.** Between M6c.5e and M6c.5f: M6c.Q1, the quality bar written; M6c.Q2a, rest checked
in CI; M6c.Q2b, counters and ratios in CI, the editor's interface measured, the first measures of
start, input, memory and power; M6c.Q3, the hierarchy's search spread over frames; M6c.Q4, fuzzing
and crash reports. After M6c.8, M6c.Q5, the stage's audit, before M6c.9. Release 0.7 comes later
for it, as the owner accepted.

## Alternatives considered

- **The figures left in the ADRs that decided them**: nothing to move, but a reader gathers ten
  ADRs, and learns that a figure changed, as ADR-0114's high tier did, only by following
  amendments.
- **Absolute times in CI, with a margin**: shared runners vary from one run to the next more than a
  milestone's gains ([ADR-0104](0104-the-build-ci-and-measurements.md)), and lavapipe's GPU is a
  CPU; a budget would fail at random, or be loosened until it meant nothing.
- **A runner of CI on the owner's machine**: true figures at each push, but the owner's machine
  running a public repository's code, and powered for every push.
- **Rest judged by CPU time**, as `top` shows it: a program that wakes a hundred times a second for
  a few microseconds shows none.
- **A looser budget for the editor's interface**, 2 ms, or a 144 Hz frame's share: the owner chose
  1.5 ms of 4.17.
- **Input timed to the photon**, with a camera or a light sensor: the whole path, but hardware, and
  by hand; measured inside the engine, it is repeatable, and the rest is stated.

## Consequences

- The roadmap gains its section 4, and the sections after it move by one.
- `measurements.md` holds CI's times; the debt of the 20 minutes links there.
- Several budgets stay unmeasured until their milestone: the roadmap says so.
- Each later milestone measures the budgets it touches, as
  [AGENTS.md](../../AGENTS.md#definition-of-done) requires.
- The glossary gains *quality bar*, *at rest* and *own wake-up*.
