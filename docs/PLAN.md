# Plan

This file says what is being worked on now: the current stage, its milestones, their status and
how each one is checked. A *stage* is a step of the roadmap; a *milestone* is a small, verifiable
step of a stage.

- The known debt still open: [`debt.md`](debt.md).
- The finished stages and their releases: [`history.md`](history.md).
- Direction beyond this stage: [`ROADMAP.md`](ROADMAP.md).
- Decisions: [`adr/`](adr/README.md).

Every milestone ends with the definition of done in [`AGENTS.md`](../AGENTS.md#definition-of-done):
build, tests, clippy, rustfmt and rustdoc pass, the program was actually run, and the affected
documentation was updated and reread. Every milestone is also checked on Windows
([ADR-0014](adr/0014-amend-roadmap-for-stage-2-and-windows.md)).

## Current

- Stage in progress: Stage 6d, uwui leaving the engine, released with 0.7, planned with the owner on
  2026-11-06 ([ADR-0194](adr/0194-stage-6d-uwui-leaving-the-engine.md),
  [ADR-0195](adr/0195-a-series-of-headless-compositors.md),
  [ADR-0196](adr/0196-checks-by-hand-on-macos.md),
  [ADR-0197](adr/0197-uwuis-repository-public-from-now.md),
  [ADR-0198](adr/0198-screens-of-reference-and-the-rivals-measured.md),
  [ADR-0199](adr/0199-uwuis-targets-tools-and-references-narrowed.md),
  [ADR-0200](adr/0200-applications-measured-from-outside.md),
  [ADR-0201](adr/0201-the-rivals-narrowed.md),
  [ADR-0202](adr/0202-how-m6d1-compares-ways-of-writing-an-application.md),
  [ADR-0203](adr/0203-pilots-replaced-pluto-and-a-daw.md),
  [ADR-0204](adr/0204-signals-written-by-hand-for-the-prototypes.md),
  [ADR-0205](adr/0205-signals-change-uwuis-interface.md),
  [ADR-0206](adr/0206-slatty-ported-again-the-daw-later.md),
  [ADR-0210](adr/0210-signals-named-for-a-reload.md),
  [ADR-0211](adr/0211-the-executor-and-other-threads-through-channels.md),
  [ADR-0212](adr/0212-a-functions-body-reloaded-through-subsecond-and-dx.md),
  [ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md),
  [ADR-0214](adr/0214-the-engine-under-a-licence-of-its-own-with-a-royalty.md),
  [ADR-0215](adr/0215-the-engine-measured-against-the-market.md),
  [ADR-0216](adr/0216-light-geometry-and-the-gpus-floor.md),
  [ADR-0217](adr/0217-worlds-by-density-generated-and-destroyed.md),
  [ADR-0218](adr/0218-one-core-for-agents-in-uwui.md),
  [ADR-0219](adr/0219-test-games-in-place-of-hunna-wood.md),
  [ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md)).
- Widened on 2026-11-07 by the market's decisions
  ([ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md#decision)): M6d.1d, M6d.2, M6d.3,
  M6d.4, M6d.5, M6d.7, M6d.10 and M6d.11 gain what those decisions need of them; M6d.9 is cut in
  two ([ADR-0218](adr/0218-one-core-for-agents-in-uwui.md)).
- Done: [M6d.0a](#m6d0a--the-screens-and-the-owners-programs),
  [M6d.0b](#m6d0b--measured-from-outside-and-ten-hello-world) and
  [M6d.0c](#m6d0c--the-applications-of-the-market), the figures before, cut in three at its start
  ([ADR-0198](adr/0198-screens-of-reference-and-the-rivals-measured.md)).
- In progress: [M6d.1](#m6d1--how-an-application-is-written), cut in four at its start; done,
  [M6d.1a](#m6d1a--the-survey-and-the-protocol) and
  [M6d.1b](#m6d1b--how-state-changes-the-interface), signals chosen with live values
  ([ADR-0205](adr/0205-signals-change-uwuis-interface.md)), checked against Slatty's list
  ([ADR-0206](adr/0206-slatty-ported-again-the-daw-later.md)); then
  [M6d.1c](#m6d1c--rusts-code-reloaded-and-the-executor), on signals, cut in three at its start
  ([ADR-0210](adr/0210-signals-named-for-a-reload.md)): done,
  [M6d.1c1](#m6d1c1--signals-named-and-a-types-state-migrated), signals named by their authors and
  kept through `serde`, and [M6d.1c2](#m6d1c2--the-executor-and-other-threads), the interface's
  executor and other threads through channels
  ([ADR-0211](adr/0211-the-executor-and-other-threads-through-channels.md)); then M6d.1c3, cut in
  two at its start ([ADR-0212](adr/0212-a-functions-body-reloaded-through-subsecond-and-dx.md)):
  done, [M6d.1c3a](#m6d1c3a--a-functions-body-reloaded-through-subsecond), a function's body
  reloaded through `subsecond` and `dx`; next,
  [M6d.1c3b](#m6d1c3b--the-rivals-reloads-timed), the rivals' reloads, which closes M6d.1c. Then
  [C1](#c1--compiling-within-its-budgets), the engine's compiling within its budgets, then
  [M6d.1d](#m6d1d--layouts-styles-and-the-host).
- The bench's branch, `m6d1b-bench`, waits for the owner's go to be deleted: uwui's measurements
  link to its code. M6d.1c's prototypes go on from it, on `m6d1c-bench`.
- [uwui's repository](https://github.com/HoneySlatt/uwui) holds its direction, its quality bar and its open decisions; its code
  moves there at M6d.11.
- May be cut at its start: M6d.5, AccessKit's translation apart.
- Stage 6c, paused since 2026-11-06, M6c.Q2e its last milestone done: its plan is in
  [`stage-6c.md`](stage-6c.md). It resumes after Stage 6e, the editor moved onto uwui's 1.0
  ([ADR-0190](adr/0190-uwuis-requirements.md),
  [ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md)).
- After Stage 6d: uwui's stages, in its repository, to its 1.0
  ([`ROADMAP.md`](ROADMAP.md#uwuis-stages)); then Stage 6e, the market measured, which moves
  *Hunna Wood* out for `games/open-world` and builds the tools that measure the engine against its
  rivals ([ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md),
  [`ROADMAP.md`](ROADMAP.md#stage-6e--the-market-measured)); then the rest of Stage 6c; then Stage 7, runtime systems,
  to be released as 0.8 ([`ROADMAP.md`](ROADMAP.md#stage-7--runtime-systems--08)).

## Status

| Milestone | Status |
|---|---|
| [M6d.0a — The screens and the owner's programs](#m6d0a--the-screens-and-the-owners-programs) | Done |
| [M6d.0b — Measured from outside, and ten "Hello, World!"](#m6d0b--measured-from-outside-and-ten-hello-world) | Done |
| [M6d.0c — The applications of the market](#m6d0c--the-applications-of-the-market) | Done |
| [M6d.1a — The survey and the protocol](#m6d1a--the-survey-and-the-protocol) | Done |
| [M6d.1b1 — The ideal code](#m6d1b1--the-ideal-code) | Done |
| [M6d.1b2 — The bench, on the control](#m6d1b2--the-bench-on-the-control) | Done |
| [M6d.1b3a — Three models on the bench](#m6d1b3a--three-models-on-the-bench) | Done |
| [M6d.1b3b — The choice](#m6d1b3b--the-choice) | Done |
| [M6d.1c1 — Signals named, and a type's state migrated](#m6d1c1--signals-named-and-a-types-state-migrated) | Done |
| [M6d.1c2 — The executor, and other threads](#m6d1c2--the-executor-and-other-threads) | Done |
| [M6d.1c3a — A function's body reloaded through subsecond](#m6d1c3a--a-functions-body-reloaded-through-subsecond) | Done |
| [M6d.1c3b — The rivals' reloads, timed](#m6d1c3b--the-rivals-reloads-timed) | Planned |
| [C1 — Compiling within its budgets](#c1--compiling-within-its-budgets) | Planned |
| [M6d.1d — Layouts, styles and the host](#m6d1d--layouts-styles-and-the-host) | Planned |
| [M6d.2 — Typed styles](#m6d2--typed-styles) | Planned |
| [M6d.3 — Documents with serde](#m6d3--documents-with-serde) | Planned |
| [M6d.4 — The canvas in uwui, on wgpu](#m6d4--the-canvas-in-uwui-on-wgpu) | Planned |
| [M6d.5 — The host's API](#m6d5--the-hosts-api) | Planned |
| [M6d.6 — A series of headless compositors](#m6d6--a-series-of-headless-compositors) | Planned |
| [M6d.7 — The desktop host: its window and its frames](#m6d7--the-desktop-host-its-window-and-its-frames) | Planned |
| [M6d.8 — The desktop host: clipboard, input methods, accessibility](#m6d8--the-desktop-host-clipboard-input-methods-accessibility) | Planned |
| [M6d.9a — One core for agents, and its MCP server](#m6d9a--one-core-for-agents-and-its-mcp-server) | Planned |
| [M6d.9b — Actions declared once](#m6d9b--actions-declared-once) | Planned |
| [M6d.10 — uwui designed as a project](#m6d10--uwui-designed-as-a-project) | Planned |
| [M6d.11 — The move](#m6d11--the-move) | Planned |
| [M6d.12 — The audit of Stage 6d](#m6d12--the-audit-of-stage-6d) | Planned |

## Stage 6d — uwui leaves the engine

Exit criterion ([`ROADMAP.md`](ROADMAP.md#stage-6d--uwui-leaves-the-engine)): uwui's code in its
repository, its CI green on Linux, Windows and macOS and its builds for the web, Android and iOS;
the engine depending on it, the editor's and the game's checks and budgets unchanged; how an
application is written decided; uwui's first stage planned.

Each milestone:

- records the design of a part in a new ADR;
- adds a dependency only when it first uses it, put to the owner first;
- keeps the editor and *Hunna Wood* working, their checks and budgets run at each push;
- measures the budgets of the [quality bar](ROADMAP.md#4-quality-bar) that it touches, and of
  [uwui's](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#2-quality-bar), and records how long CI's jobs take in
  [`measurements.md`](measurements.md), each within its limit of 20 minutes, and the times of
  compiling ([ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md));
- from M6d.4, adds to uwui's benchmarks the counters and the discriminating cases of each phase
  that it touches, checked in CI ([ADR-0199](adr/0199-uwuis-targets-tools-and-references-narrowed.md));
- may be cut in two at its start.

Figures of the programs that leave other frameworks belong to uwui: they go to its repository's
`docs/measurements.md`. GPU measures wait while the owner plays on the machine.

Out of scope: what uwui's first stage builds after the move, the trials, touch, fonts given as
bytes, the authoring API built, drawing, images, motion, components and the four ports
([uwui's roadmap](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#4-stages)); the editor moved onto uwui's components,
at Stage 6c's resumption.

Open risks:

- The canvas's layout: handing items on as instances was measured slower than reading them by
  their index, and WebGL2 allows 15 values between the shader's stages
  ([ADR-0194](adr/0194-stage-6d-uwui-leaving-the-engine.md)).
- `slime-color` derives the engine's `Reflect`: leaving with uwui, Rust's orphan rule would make the
  engine's lowest crate depend on uwui.
- Docks' layouts and shortcuts saved by users, read by a new reader.
- KWin and Mutter headless in CI: a session bus each, events dropped before a device is resumed, a
  first key lost; no tool of libei's drives them.
- The history filtered at the move must hold no personal data.

### M6d.0 — The figures before

The stage starts from figures measured, and from screens of reference chosen before any work and
never tuned for. Each screen has two figures: the owner's program gives the floor, the port's
check; the rivals give the bar to beat, the best of them criterion by criterion
([ADR-0198](adr/0198-screens-of-reference-and-the-rivals-measured.md)). The screens are written in
[uwui's quality bar](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#screens-of-reference).
The programs' and the rivals' figures go to uwui's
[`docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md); the
engine's, to [`measurements.md`](measurements.md). The other repositories are read, never written:
their programs are built from clones. Cut in three at its start.

### M6d.0a — The screens and the owner's programs

- **Goal.** The screens frozen; the floor of the owner's programs and the engine's own costs
  measured.
- **Crates.** `slime-render`'s tests; `scripts/`; Slatty's and Opium's repositories, read only.
- **Delivers.** The four screens of reference, written in uwui's quality bar. Slatty's library
  measured again by `slatty-measure`, as on 2026-11-05. Opium's table of songs measured on a fake
  library of 8,500 local songs, with Opium's own scripts: its start to its first frame, its memory
  with the table open; its rest, frames and power need a person's input to open the table without
  the screenshot mode, which repaints five times a second, and wait for M6d.0b. [ADR-0180](adr/0180-one-canvas-and-the-frameworks-foundations.md)'s
  figures: what `Renderer::new` costs an application in start, memory and video memory against the
  canvas alone; what a program on uwui compiles, its crates, its build time and its size; its two
  others, a start and the rest, at M6d.0b.
- **Done when.** Each figure is measured, or says why not; the screens are written down.
- **How to test.** `slatty-measure`, Opium's own scripts through uwui's `scripts/opium-measure.sh`,
  `scripts/canvas-cost.sh --gpu` and `scripts/uwui-program.sh`, on a machine at rest.
- **Measured.** The engine's figures in
  [`measurements.md`](measurements.md#m6d0a-the-renderer-against-the-canvas-and-a-program-on-uwui);
  Slatty's and Opium's in
  [uwui's](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md). Slatty's scrolling,
  on the owner's display, was measured with its window beside others, not alone on the output.

### M6d.0b — Measured from outside, and ten "Hello, World!"

- **Goal.** Any application measured the same way, without a probe in its code.
- **Crates.** None; uwui's `scripts/`; the programs' repositories, read only.
- **Delivers.** A method, and its scripts in uwui's repository, that read from outside an
  application's frames, start, memory, rest and power; checked against Slatty's probe. In motion,
  while it scrolls and animates, it reads too the application's CPU, thread by thread, the
  compositor's CPU, which shows a window that sends its whole content at each frame, and the GPU's
  load and power: a rate held can hide their cost. Memory after a list is scrolled and after it
  is closed; the 95th percentile of frames beside the 99th; presentations missed. A person's input given in a headless
  compositor: ADR-0191's `wlrctl` in sway moved and clicked a pointer that never reached Opium's
  window, why not established. Opium's table of songs at rest, scrolled, and its wave moving,
  measured by it. ADR-0180's two figures that M6d.0a left: the `widgets` example's start to its
  first useful frame, against the bar's budget, and its rest under Wayland and under Xwayland. gpui-kit's
  "Hello, World!", and the same written with GPUI and gpui-kit, Flutter, Compose for the desktop,
  Electron, Tauri, Qt Quick, Slint, Iced and egui, as each one's documentation shows, measured;
  and GPUI alone, the tenth ([ADR-0200](adr/0200-applications-measured-from-outside.md)).
- **Done when.** The method agrees with Slatty's probe within a stated margin; each program is
  measured, or says why not.
- **How to test.** The method run on Slatty beside its probe: uwui's `scripts/slatty-check.sh`.
- **Measured.** The method, its check against Slatty's probe, Opium's table and the ten
  "Hello, World!" in [uwui's](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md);
  the `widgets` example's start and rest in
  [`measurements.md`](measurements.md#m6d0b-a-program-on-uwuis-start-and-rest-measured-from-outside).
  The `widgets` example never rests, its progress bars moving; the processor's power is not
  measured, its sensor read by root alone. Opium's table scrolled on the owner's display, at
  M6d.0c, in
  [uwui's](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#opiums-songs-on-the-owners-display-2026-11-06):
  as smooth as in sway, Umbriel spending nearly twice sway's processor.

### M6d.0c — The applications of the market

- **Goal.** The bar to beat, measured on the same screens.
- **Crates.** None; uwui's `scripts/`.
- **Delivers.** Measured by M6d.0b's method, through uwui's `scripts/market.sh`: Steam on the
  owner's account, Nautilus and gThumb on Slatty's 10,000 covers; Spotify and Cider on the
  owner's accounts; Zed scrolling a file of 100,000 lines. Each figure names the application's
  version, its date, and the size of the library it ran on. Strawberry, Elisa, GNOME Music and
  Heroic set aside by the owner during the milestone
  ([ADR-0201](adr/0201-the-rivals-narrowed.md)).
- **Done when.** Each figure is measured, or says why not; the bar of each screen is written in
  uwui's quality bar.
- **How to test.** M6d.0b's method, pointed at each rival.
- **Measured.** The rivals and Opium's table on the owner's display in
  [uwui's](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#the-applications-of-the-market-2026-11-06);
  the bar of each screen in
  [uwui's quality bar](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#the-bar-of-each-screen).
  `measure.py`'s stillness, wrong when a busy thread ended, was fixed on the way; Electron's and
  Tauri's starts of M6d.0b, measured again, stand. Not measured: the processor's power, read by
  root alone.
- **By hand.** The owner: the very modest laptop's inventory and its measures; the music player's
  Android prototype in Compose, on the Pixel. The laptop's inventory was given on 2026-11-06, from
  `lshw -short` and `inxi`, and is written in
  [uwui's quality bar](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#machines);
  what it lacks and its measures wait for the owner (below).

### M6d.1 — How an application is written

The owner decides how an application on uwui is written, from prototypes measured on the screens
of reference. No code is kept: the prototypes live on a branch of this repository, where uwui's
tree still is, then are thrown away; their figures go to uwui's
[`docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md). Each
question is put to the owner, recorded in an ADR, and uwui's open decisions are updated as each
ADR lands. Cut in four at its start, on 2026-11-06, in the order of the questions' dependencies:
the state model decides whether Rust's code can be reloaded and who owns the interface's tasks
([ADR-0202](adr/0202-how-m6d1-compares-ways-of-writing-an-application.md)). How the models are
compared, the criteria, the screens and their scripted interactions, is that ADR's.

### M6d.1a — The survey and the protocol

- **Goal.** What the field does on each question, and how the models will be compared.
- **Crates.** None; a document in uwui's repository.
- **Delivers.** [The survey](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md),
  each reference read on what it brings, each fact with its source, version and date: Floem for
  signals; GPUI with gpui-kit for views that say they changed, and its executor; Iced for
  messages; Makepad and Slint for the format of layouts; Dioxus, Flutter and Jetpack Compose for
  Rust's code reloaded; [Rune](https://github.com/anomalyco/rune) for motion that runs no view
  again; SwiftUI read only, without a Mac to measure it
  ([ADR-0199](adr/0199-uwuis-targets-tools-and-references-narrowed.md)). The protocol of
  comparison, put to the owner one question at a time and recorded in
  [ADR-0202](adr/0202-how-m6d1-compares-ways-of-writing-an-application.md).
- **Done when.** The survey covers each question; the protocol is decided and recorded.
- **Measured.** Nothing: a milestone of documents. The references' figures in the survey are their
  authors'.

### M6d.1b — How state changes the interface

The owner chooses the state model from four, today's rebuild by keys as the control, signals,
views that say they changed, and messages, each prototyped over uwui's tree and measured as
[ADR-0202](adr/0202-how-m6d1-compares-ways-of-writing-an-application.md) says. Cut in three at its
start, on 2026-11-06: the ideal code is put to the owner before any prototype, and the bench is
checked on the control before the other models are compared on it.

ADR-0202's method, refined at its start:

- Counts, of work, allocations, bytes and items dirtied, are exact: the prototypes run on one
  thread, on data made in the program, without chance. Each is taken once, and checked equal on a
  second run, which shows any that is not. Times are the median of 21 runs, with their range, in
  release builds on the high tier's machine: enough runs for a median that a few slow ones do not
  move, in seconds.
- Lines are counted after rustfmt with its defaults, edition 2024, by uwui's
  [`scripts/count_code.py`](https://github.com/HoneySlatt/uwui/blob/main/scripts/count_code.py);
  concepts as each file's header lists them, one bullet each.
- Whether a model allows each reload is answered without `subsecond` or `serde`, whose ADRs are
  M6d.1c's: a function's body by a table of the entry points' addresses that the test swaps, as
  `subsecond` does; a type's state by a form that does not depend on its layout, written by hand
  and read into the changed type. The state is then checked: the scroll, the selection, a field's
  text, the filter and a page open. How long a reload takes is M6d.1c's.

### M6d.1b1 — The ideal code

- **Goal.** Each screen's code as an application's author would want to write it, in each model,
  before any prototype.
- **Crates.** None; files in uwui's repository.
- **Delivers.** For each of the four models and the four screens of
  [ADR-0202](adr/0202-how-m6d1-compares-ways-of-writing-an-application.md), in uwui's
  [`docs/ideal-code/`](https://github.com/HoneySlatt/uwui/blob/main/docs/ideal-code/README.md),
  the screen's code, formatted by rustfmt, its lines and its concepts counted; beside it, the
  references' own examples of the same kind of screen, their lines and concepts. ADR-0202's method
  reviewed.
- **Done when.** The owner has read the code and agreed that the prototypes start: done on
  2026-11-06.
- **Measured.** Lines and concepts, in the ideal code's README; no program runs.

### M6d.1b2 — The bench, on the control

- **Goal.** ADR-0202's instruments, checked on today's model before the others are compared on
  them.
- **Crates.** None kept but the benchmarks' allocator, which counts bytes too; the rest on a
  branch, thrown away.
- **Delivers.** Each phase of a frame timed and counted; the display list's items changed, out of
  all; the time from an input to the display list that shows it, on a clock that the test moves;
  the memory held open and after a close; a frame where nothing changed; the display list compared
  with a new interface's after each scripted interaction; the screens' data made in the program.
  What the four models share, built once on the branch, as
  [the ideal code](https://github.com/HoneySlatt/uwui/blob/main/docs/ideal-code/README.md#what-the-prototypes-must-add-to-uwuis-tree)
  lists it: elements made without a style, a grid, the selection of a grid and a list, a theme by
  its name, frames asked for, each frame's time, an element's style as it appears, the wave. The
  control prototyped from its ideal code and measured on each screen. The command that runs
  the measures, documented for the owner's very modest laptop.
- **Done when.** The control's figures are in uwui's
  [`docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md), and
  the bench shows the control's own mistake, a revision not moved, as a wrong render: done on
  2026-11-06.
- **Measured.** Each criterion of ADR-0202 for the machine, for the control, without a window:
  [the control's figures](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#the-control-on-m6d1bs-bench-2026-11-06),
  with how the bench measures, the command for the owner's laptop, and the lines that the
  prototype forced to change.
- **Where.** The bench and the control's prototype on the branch
  [`m6d1b-bench`](https://github.com/HoneySlatt/SlimeEngine/tree/m6d1b-bench/benches/src/prototypes),
  `slime-benches --prototypes`, which M6d.1b3a extends and which is thrown away once M6d.1b ends;
  on `main`, the benchmarks' allocator, which counts bytes too.

### M6d.1b3a — Three models on the bench

M6d.1b's third part, cut in two at its start, on 2026-11-06: the three models measured together on
one bench, then the choice, which waits for what Opium's, Pluto's and the DAW's long-term
directions ask of uwui ([ADR-0203](adr/0203-pilots-replaced-pluto-and-a-daw.md)).

- **Goal.** Signals, views that say they changed, and messages, measured on the control's bench.
- **Crates.** None kept.
- **Delivers.** The three prototyped on the bench from their ideal code, the lines that it forced
  to change counted; the signals' runtime written by hand for the prototypes, no library added
  ([ADR-0204](adr/0204-signals-written-by-hand-for-the-prototypes.md)). Elements nested as tuples, which the three write and the control
  does not, added to the bench's shared parts; the bench's delay ended by each model's own pending
  work too, not only by a builder asked. Each model's small cases of wrong renders and panics. In
  each, whether a function's body can be reloaded, a type's state migrated, and a task cancelled
  with its component. What moving the editor and *Hunna Wood* to each model would cost, counted in
  their code.
- **Done when.** The three models' figures are in uwui's
  [`docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md),
  beside the control's, and each mistake of the ideal code's README shows as a wrong render or a
  panic: done on 2026-11-07, but for three, each shown otherwise. A key forgotten passes the
  focus to another item, which the bench's comparison does not see; a subscription not given up
  costs at rest; and the control's id kept after its builder ran again is no mistake, its
  generation keeping it from naming another element.
- **Measured.** Each criterion of ADR-0202, for each model, screen and scripted interaction,
  without a window:
  [the four models' figures](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#three-models-beside-the-control-on-m6d1bs-bench-2026-11-07),
  with what the bench added, the mistakes, the reloads, a task cancelled, the lines forced to
  change and the move's cost.
- **Where.** The prototypes on the branch
  [`m6d1b-bench`](https://github.com/HoneySlatt/SlimeEngine/tree/m6d1b-bench/benches/src/prototypes),
  thrown away once M6d.1b ends.

### M6d.1b3b — The choice

- **Goal.** The owner chooses the state model.
- **Crates.** None kept.
- **Delivers.** What the lists of Opium, Pluto's client and the DAW, each of what it asks of uwui,
  mark as general, read in their repositories once written and pushed
  ([ADR-0203](adr/0203-pilots-replaced-pluto-and-a-daw.md)): for each model, whether it covers
  each such need, text that streams in, data in real time at 240 Hz, timelines of great length,
  long tasks cancelled and several windows among them. A need that no
  model covers is put to the owner, with options, before the choice. The screens of reference and
  ADR-0202's criteria stay as they are. The figures of M6d.1b3a put to the owner.
- **Done when.** The model is decided and recorded in an ADR; uwui's open decisions are updated:
  done on 2026-11-07. Signals, with live values, a primitive of uwui's tree for the frame's values
  that no model covered ([ADR-0205](adr/0205-signals-change-uwuis-interface.md)).
- **Slatty's check.** Slatty, back among uwui's first programs
  ([ADR-0206](adr/0206-slatty-ported-again-the-daw-later.md)), wrote its list after its interview,
  in [its repository's `docs/uwui-needs.md`](https://github.com/HoneySlatt/Slatty/blob/main/docs/uwui-needs.md).
  Each need that it marks as general was checked against signals and live values on 2026-11-07:
  none left uncovered. Three rest on what ADR-0205 leaves to later milestones: changes from other
  threads gathered into the next frame and a signal's state migrated across a reload (M6d.1c),
  and timers at the granularity that they show (uwui's first stage). The one that it marks new,
  signals fed by a state outside uwui in batches, by parts and by key, is a list fed by its
  changes and a batch of sets gathered into the frame, as the prototype's bindings wait for the
  frame's build.
- **The engine's check.** The engine's own list, [`uwui-needs.md`](uwui-needs.md), was checked the
  same way on 2026-11-07: none left uncovered. A HUD's values at each frame are live values; the
  editor's hierarchy of 100,000 entities is a list fed by its changes by key; a clock for each root,
  the game's paused or slowed, feeds signals' timers and motion; an interface's state kept in a
  save and rewound rests, as the reload does, on a signal's state named (M6d.1c).
- **Measured.** Nothing new: M6d.1b3a's figures.

### M6d.1c — Rust's code reloaded, and the executor

Rust's code reloaded with its state kept, and the interface's tasks, on signals. The prototypes go
on from M6d.1b's bench, on the branch `m6d1c-bench`, thrown away once M6d.1 ends. Cut in three at
its start, on 2026-11-07, one question and one review each, in the order that ADR-0210 gives
([ADR-0210](adr/0210-signals-named-for-a-reload.md)); M6d.1c3 cut in two at its start, on the same
day ([ADR-0212](adr/0212-a-functions-body-reloaded-through-subsecond-and-dx.md)).

### M6d.1c1 — Signals named, and a type's state migrated

- **Goal.** A signal's state kept across a reload of its type.
- **Crates.** None kept. On the branch, `slime-benches` takes `serde` and `serde_json`, already in
  the lock file.
- **Delivers.** Signals named by their authors in their scopes, `RwSignal::kept` and
  `Stored::kept`, their values written through `serde` by their scopes' paths and their names, and
  read again into their types; a signal not named starts again. Shown by tests on Slatty's library:
  a body changed, a signal added before the others, a signal renamed, a value that no longer
  reads; and apart: a field added to a kept type, a grid's cell whose state follows its item, a
  document's lines kept with their signals, a read that fails midway, two names alike, a value
  that cannot be written; and the two limits of a scope's path, a part used twice in one scope and
  an element added before a dynamic view.
- **Done when.** Decided and recorded: done on 2026-11-07, in
  [ADR-0210](adr/0210-signals-named-for-a-reload.md).
- **Measured.** The reload timed in the program, from its call to the frame that shows it, without
  the compiler's part, which M6d.1c3a times; the state kept; what names cost the bench's screens.
  In [uwui's `docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#signals-named-across-a-reload-2026-11-07).

### M6d.1c2 — The executor, and other threads

- **Goal.** The interface's tasks, and other threads' changes, on signals.
- **Crates.** None kept.
- **Delivers.** The interface's executor, its tasks bound to components and cancelled with them,
  run on time simulated in tests, and its waker; other threads' changes to signals gathered into
  the next frame, the interface's thread never waiting; another thread's handle on a signal kept
  valid across a reload.
- **Done when.** Each question is decided and recorded: done on 2026-11-07, in
  [ADR-0211](adr/0211-the-executor-and-other-threads-through-channels.md). Tasks owned by their
  scopes, their timers on the frame's clock, the host waiting for the next deadline; other threads
  holding a channel's sender, never a signal; the receiver made outside the view, so that a reload
  keeps it. Shown by tests: on Slatty's downloads, 10,000 games whose progress another thread
  sends, a burst and a batch shown whole in the next frame, and a sender valid across a reload,
  nothing lost; apart, a timer that the host waits for, a frame at rest while 1,000 tasks wait,
  senders on four threads while frames run, a task's budget of values a poll, a channel made
  within a view closed with it, a task cancelled in its own poll, and an application closed with
  tasks waiting.
- **Measured.** A change from another thread to the frame that shows it; what a frame at rest costs
  with tasks waiting. In
  [uwui's `docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#the-interfaces-executor-and-other-threads-2026-11-07).

### M6d.1c3a — A function's body reloaded through subsecond

- **Goal.** A function's body reloaded with its state kept, timed.
- **Crates.** None kept. On the branch, `slime-benches` takes `subsecond` and `tungstenite` behind
  its feature `hot`, and its thread-locals move to a crate of their own, `slime-bench-locals`.
- **Delivers.** Slatty's library reloaded under `dx serve --hot-patch`, its view's body changed
  and saved by the bench, the patch applied between two frames and the view run again through
  `subsecond`'s table, the kept signals keeping the state; on a crate of the size of Slatty's
  interface, views of the library's shape added by its `build.rs`. Each run fails if the frame
  does not show the change with the scroll, the selection, the filter and the page kept. The same
  change built again by cargo, timed beside it. Whether the reload works on the web and on
  Android, weighed from the survey.
- **Done when.** Decided and recorded: done on 2026-11-07, in
  [ADR-0212](adr/0212-a-functions-body-reloaded-through-subsecond-and-dx.md). A body reloaded
  through `subsecond`, its patches compiled and sent by `dx`, applied by the host between two
  frames; `uwui dev` compiling with line tables alone, which keeps the budget of a change seen;
  no thread-local in the crates that a patch links again, uwui's reached through functions
  neither generic nor inlined; the web and Android covered by `subsecond` and `dx`, not tried.
- **Measured.** The time from a change saved to the frame that shows it, at two sizes and two
  levels of debug information; the state kept across each reload; the same change built again by
  cargo. In
  [uwui's `docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#a-functions-body-reloaded-through-subsecond-2026-11-07).

### M6d.1c3b — The rivals' reloads, timed

- **Goal.** The rivals' reloads of a function's body timed, beside uwui's.
- **Crates.** None kept. In uwui's repository, a step of `scripts/measure.py`.
- **Delivers.** A step of `measure.py` that saves a change to a file and times the first frame
  that the compositor shows changed; the reloads of Dioxus, Iced, GPUI, Makepad, Slint, Flutter and
  Compose Hot Reload timed with it on their own examples, as each runs here: uwui's frozen
  "Hello, World!" where one exists, Dioxus's and Makepad's added. Each download outside the
  package managers put to the owner first.
- **Done when.** Each rival's reload timed, or why it could not be; uwui's open decision on
  Rust's code closed; M6d.1c marked done.
- **Measured.** From a change saved to the first frame that shows it, the median of 21; the state
  that each keeps.

### C1 — Compiling within its budgets

Not a milestone of uwui's: the engine's compiling, between M6d.1c3b and M6d.1d, as
[ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md) has it done now.

- **Goal.** A change of a game's code to the game running again within its budget in a debug build,
  and every budget of compiling measured ([`ROADMAP.md`](ROADMAP.md#budgets)).
- **Crates.** The workspace's `Cargo.toml` and profiles; no crate's code.
- **Delivers.** One set of features for the whole workspace, so that an engine change is compiled
  once for the game and the editor; dependencies optimised in debug builds; a faster linker chosen
  by measure, put to the owner as a dependency before it is added; a profile for shipping, with
  optimisation across crates; the times of compiling measured.
- **Done when.** The figures are in [`measurements.md`](measurements.md), each budget within or its
  debt written in [`debt.md`](debt.md); CI is green on Linux and Windows.
- **Measured.** The steps of the section "Compiling, Ryzen 7 5800X" of `measurements.md`, before and
  after, and the shipping build.

### M6d.1d — Layouts, styles and the host

- **Goal.** The rest of how an application is written, and what M6d.5's host API takes from it.
- **Crates.** None kept.
- **Delivers.** The format of layouts and their reload, with a style's; styles written in code
  beside the themes' files; the time of each frame that motion reads. What a theme is: its values,
  its rules and the components' structure, which a theme may change, so that uwui's official
  themes are crates of their own ([ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md)).
  An application started in one call, which names its look, uwui's own, Glass, Material 3
  Expressive, Pro or blank, none chosen in silence
  ([ADR-0215](adr/0215-the-engine-measured-against-the-market.md)). M6d.5 updated.
- **Done when.** Each question is decided and recorded; uwui's open decisions on how an
  application is written are all closed; a theme that changes a component's structure is written
  in the decision's examples.
- **Measured.** A layout's and a style's change seen, against the budget of 2 s.

### M6d.2 — Typed styles

- **Goal.** uwui's styles cascade as typed values, without the engine's reflection.
- **Crates.** `uwui`, `slime-editor`, `hunnawood-editor`.
- **Delivers.** Styles typed per component, each part of a style a type of its own, each field
  optional, merged by the cascade, with parts that a theme extends; the themes' named values
  resolved by type; no colour hidden in a component's defaults, each taken from its theme; styles
  written in code as M6d.1 decides
  ([ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md)).
- **Done when.** A property test: any style written as before reads the same through the typed
  cascade; a test finds no colour in the components' defaults; a theme extends a component's style
  with a part of its own; the look's image tests and counters are unchanged; CI is green.
- **How to test.** `cargo test -p uwui -p slime-editor`; `scripts/check.sh`.
- **Measured.** The editor's interface and its counters; a frame that styles again after a change
  of theme.

### M6d.3 — Documents with serde

- **Goal.** uwui reads and writes its documents with `serde`, and the files that users saved open
  as before.
- **Crates.** `uwui`, `slime-cook`, `slime-editor`, `slime-color`, `hunnawood`.
- **Delivers.** Themes, style sheets, shortcuts and docks' layouts read and written by `serde` and
  `toml`, behind a feature; without it, a theme written in code. A primary modifier, Ctrl on Linux
  and Windows, Cmd on macOS, in the format of shortcuts, so that the shortcuts that users saved
  migrate once; a menu's shortcut shown as its platform writes it
  ([ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md),
  [ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md)). Decided at its start: the cooked
  form of these documents, and `slime-color`'s home with its reflection.
- **Done when.** Every document of the repository, and layouts and shortcuts saved by the editor
  kept as fixtures, read the same as before; a shortcut saved with Ctrl reads as the primary
  modifier, and its menu shows it as Linux, Windows and macOS write it; `cargo tree -p uwui` holds
  neither `slime-reflect` nor `slime-serialize`; CI is green.
- **How to test.** `cargo test -p uwui -p slime-cook -p slime-editor`.
- **Measured.** Reading the editor's documents at its start.

### M6d.4 — The canvas in uwui, on wgpu

- **Goal.** uwui draws itself with a wgpu device and a target, without the engine's renderer, on a
  path that WebGL2 takes.
- **Crates.** `uwui-render`, `slime-render`, `slime-gfx`, `slime-app`, `slime-devui`,
  `slime-editor`, `hunnawood`; `AGENTS.md`.
- **Delivers.** First, a prototype thrown away: items handed on as flat values of instances,
  against items kept in a texture that the shader reads by their index, each counted against
  WebGL2's limits for the drawing that uwui's first stage plans, and measured; the prototype holds
  a blur of what lies behind, a group drawn apart, a transform per item and a path, and its ADR
  records the canvas's passes ([ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md)). Then
  the canvas in `uwui-render` on the layout chosen, taking a device, its queue and a target; the
  engine handing it its device and composing it over its views; image tests on wgpu's GL backend
  at WebGL2's limits, if the owner accepts wgpu's `gles` feature; AGENTS.md's rule on wgpu's types
  naming `uwui-render`.
- **Done when.** The prototype's blur, group, transform and path are measured on both layouts, and
  the canvas's ADR records its passes; the editor, the dev overlay and *Hunna Wood*'s menus draw as
  before in their image tests; a test draws a tree of uwui with a bare wgpu device; CI is green.
- **How to test.** `cargo test --workspace`; `scripts/check.sh`.
- **Measured.** `render.canvas` and `ui.draw.menu`, before and after, at 3840 × 2160 on the
  RX 6900 XT, within the interface's 0.5 ms.

### M6d.5 — The host's API

- **Goal.** A host drives uwui through one explicit API, and the engine is its first host.
- **Crates.** `uwui`, `slime-app`, `slime-platform`.
- **Delivers.** As [ADR-0181](adr/0181-uwuis-scope.md) decides and M6d.1 shapes: the surface
  created, lost, made again and resized, with its scale; the pointer, multi-touch, the keyboard and
  gamepads as events; the bridge to input methods; the lifecycle; the clipboard; requests for
  frames and the time of uwui's next change, its tasks' timers among them
  ([ADR-0211](adr/0211-the-executor-and-other-threads-through-channels.md)), the time of a frame,
  and a waker from any thread; the translation of meaning into
  AccessKit's tree moved into uwui; `slime-app` a host through this API alone. The API checked,
  item by item, against winit, Android, the web, UIKit and AppKit: a surface for each root, input
  methods both ways, the insets, the system's preferences, an application's events, and a model of
  menus that macOS's menu bar takes
  ([ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md),
  [ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md)).
- **Done when.** `slime-app` reaches uwui through the API alone; a table in the API's ADR says, for
  each item and each of the five, how it maps or why it needs no mapping; the editor's and the
  game's sessions, rest and input to present are unchanged; CI is green.
- **How to test.** `cargo test --workspace`; `scripts/check.sh`.
- **Measured.** Rest; input to present; the editor's start.

### M6d.6 — A series of headless compositors

- **Goal.** The editor and the game are checked on headless Weston, sway, KWin, Mutter and X11
  through Xvfb, and what differs is written down.
- **Crates.** `slime-mcp`; `scripts/`, `flake.nix`, `.github/workflows/`.
- **Delivers.** As [ADR-0195](adr/0195-a-series-of-headless-compositors.md) decides: a development
  shell and a job of CI of their own, run when the platform layer or the desktop host changes,
  every week and on demand; input for each family, the dependency of libei's client put to the
  owner; rest, an agent's session, decorations, the scale, frames and presentation times, the
  clipboard and the cursor's shapes checked on each; each compositor's protocols listed by
  `wayland-info`; a table of what differs.
- **Done when.** Each compositor runs the checks, or its report says why not; the job is within 20
  minutes; CI is green.
- **How to test.** The new job, run on demand.
- **Measured.** The job's time; what each compositor adds to the store.

### M6d.7 — The desktop host: its window and its frames

- **Goal.** An application runs on uwui alone: its window, frames at the display's rate, none at
  rest, on every compositor of the series.
- **Crates.** uwui's desktop host, a new crate; `uwui`; an example.
- **Delivers.** The window's identity, its minimum size, fullscreen, brought to the front, its
  closing intercepted; decorations of the system where the compositor draws them, drawn by uwui
  elsewhere. The ADR of the frames' rate, which amends [ADR-0021](adr/0021-frame-pacing.md) and
  which M6c.Q2f waits for: on the compositor's signal on Wayland, by the refresh rate of the
  display that holds the window on Windows and X11, a display that reports none included. A waker
  from any thread; an example; a check that a program on uwui and its host compiles neither the
  renderer, the world nor the editor; a job of CI that builds and tests uwui and its host on macOS
  ([ADR-0173](adr/0173-the-platforms-of-applications.md)). A *blank window*, a program on winit and
  wgpu that only clears its window and presents it, among the programs measured
  ([ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md)). The host's crash reports,
  written on the machine and never sent
  ([ADR-0215](adr/0215-the-engine-measured-against-the-market.md)).
- **Done when.** The example rests on every compositor of the series; its input to present is
  within 4.17 ms in a headless sway at 240 Hz; the check fails on a program made to pull
  `slime-render`; a program on the host made to crash leaves its report on the machine; CI is
  green, macOS included.
- **How to test.** `cargo test -p uwui`; the series' job; `scripts/input-to-present.sh`.
- **Measured.** The host's own cost, what the example adds above the blank window, in the same run:
  start, memory, the binary's size, rest, input to present. The applications' budgets get first
  figures, not judged on the example.
- **By hand.** The owner's desktop; the Windows virtual machine; macOS 26
  ([ADR-0196](adr/0196-checks-by-hand-on-macos.md)).

### M6d.8 — The desktop host: clipboard, input methods, accessibility

- **Goal.** Copying and pasting, typing through an input method, and screen readers work on every
  compositor of the series.
- **Crates.** uwui's desktop host; `uwui`.
- **Delivers.** A clipboard that every compositor serves, its design in an ADR; input methods;
  AccessKit's adapter.
- **Done when.** Copy and paste are tested on each compositor, Mutter's and Weston's included; CI is
  green.
- **How to test.** The series' job.
- **By hand.** Orca on the owner's desktop, Narrator in the Windows virtual machine, VoiceOver and
  text input on macOS 26.

### M6d.9 — Agents reach an application

One core for agents, in uwui, under MIT OR Apache-2.0, which the engine builds its own families on
([ADR-0218](adr/0218-one-core-for-agents-in-uwui.md)). Cut in two by that ADR.

### M6d.9a — One core for agents, and its MCP server

- **Goal.** An agent, named and consented to, reads and drives an application on uwui, and the
  engine reaches its editor and games through the same core.
- **Crates.** uwui's core for agents and its MCP server, new crates; `slime-channel`, `slime-mcp`.
- **Delivers.** A protocol that no transport binds, so that Android and the web can carry it later;
  a first message that names the agent, with the person's consent agent by agent; numbered
  requests, and events that the program sends; attaching to a program already running; rights for
  each agent, a preview before applying, and tools found by families
  ([ADR-0209](adr/0209-platforms-the-editor-and-agents.md),
  [ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md)); uwui's MCP server built on
  it. The engine's channel speaking the core's protocol, and `slime-mcp` on uwui's MCP library with
  the engine's families: the world, play and editing.
- **Done when.** A named agent attaches to the example already running in a headless Weston and
  drives it; a request outside its rights is refused, and one asked with a preview changes nothing
  until applied; the engine's MCP session tests pass through the core; CI is green.
- **How to test.** The servers' session tests, uwui's and the engine's.
- **Measured.** An answer of the channel, and an event from the program to the agent; the engine's
  answer against its figure before.

### M6d.9b — Actions declared once

- **Goal.** An application declares its actions once for agents, and the editor's operations are
  the declaration's first client.
- **Crates.** uwui's core for agents; `slime-editor`.
- **Delivers.** Actions declared once: a name, typed parameters, a description, a permission and a
  family. The declaration checked on paper against Android's AppFunctions, Apple's App Intents and
  Windows' registry of agents, so that an adapter never changes it
  ([ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md),
  [ADR-0218](adr/0218-one-core-for-agents-in-uwui.md)). The editor's operations declared: what each
  changes becomes its permission and family, its parameters typed, each runnable without a window.
- **Done when.** The comparison with the three systems is written in the declaration's ADR; every
  operation of the editor is declared, and an agent runs each one without a window through the
  MCP server; CI is green.
- **How to test.** `cargo test -p slime-editor`; the MCP server's session tests.
- **Measured.** The MCP server's start with the editor's actions declared.

### M6d.10 — uwui designed as a project

- **Goal.** uwui's repository is ready to receive its code. Builds nothing.
- **Delivers.** Put to the owner, then recorded: uwui's first stage, its milestones and their order,
  the trials before the authoring API is built proposed; how its ADRs are numbered; the moved
  crates' names; how the engine follows uwui after the move; which repository holds the
  interface's budgets; what stands for the engine in uwui, its benchmarks and a menu driven by a
  gamepad; how the engine keeps a series of compositors for its platform layer; the measure that
  confirms games' interfaces on uwui's core with a layer of the engine's, or gives them a
  framework of their own if it fails
  ([ADR-0209](adr/0209-platforms-the-editor-and-agents.md)), as the engine's
  [`uwui-needs.md`](uwui-needs.md#for-m6d10) proposes it. What the engine, Opium, Pluto's client,
  Slatty and Magnolia ask of uwui weighed for its first stage, each list's general needs apart. MyGo read for its contracts with the platforms and its
  delivery. A matrix of uwui's platforms and functions, each available, experimental, planned or
  not checked ([ADR-0199](adr/0199-uwuis-targets-tools-and-references-narrowed.md)). Every
  decision of [ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md) to
  [ADR-0219](adr/0219-test-games-in-place-of-hunna-wood.md) weighed for uwui's first stage; among
  them, the services of files, settings, the network and long work that the web and the phones
  lack, with [ADR-0211](adr/0211-the-executor-and-other-threads-through-channels.md) amended for
  the web. Who owns commands, undo and their journal, uwui or the engine, now that the actions are
  declared ([ADR-0218](adr/0218-one-core-for-agents-in-uwui.md)). uwui's `AGENTS.md`, roadmap and
  plan updated.
- **Done when.** The owner has validated uwui's documents; each decision is recorded in an ADR.

### M6d.11 — The move

- **Goal.** uwui's code lives in its repository, and the engine depends on it.
- **Delivers.** The history of uwui's files filtered from a clone by `git filter-repo`, following
  their former paths, and merged into uwui's repository, checked for personal data, authored by
  the owner's identity alone, its links to the engine's ADRs made absolute; the crates that leave
  with it, as M6d.10 decides, under MIT OR Apache-2.0 and free of the engine's crates
  ([ADR-0214](adr/0214-the-engine-under-a-licence-of-its-own-with-a-royalty.md)); its CI on Linux,
  Windows and macOS, and its builds for the web, Android and iOS; the editor's look moved into the
  official theme Pro, a crate of its own in uwui's repository, its images of reference in uwui's CI
  ([ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md)); the engine depending on it;
  uwui's files removed here, every document updated.
- **Done when.** Both repositories' CI are green; the editor draws as before on the theme Pro, in
  its image tests; the engine's budgets are unchanged.
- **Measured.** Both CIs' times; the engine's build time.

### M6d.12 — The audit of Stage 6d

- **Goal.** Both repositories examined in every dimension before Stage 6d closes.
- **Delivers.** As [ADR-0177](adr/0177-audits-and-releases.md) decides: its scope, a reviewing agent
  for each dimension, each finding verified and put to the owner; every budget measured again;
  the record, `docs/audits/<date>-stage-6d.md`; the fixes done or planned in
  [`debt.md`](debt.md).
- **Done when.** The record is written; the fixes that the owner chose are done; CI is green.

## Checks by hand waiting for the owner

The owner decided on 2026-10-20 to make the checks by hand later, together. Each milestone adds its
own here when it is done. A check leaves the list once made, its result written in its milestone
while the stage is in progress, then in the stage's entry of [`history.md`](history.md). The game
is run with `--new --save <a file of its own>`, so that the owner's save is left alone. A check
made after Stage 6e's first milestone runs on `games/open-world` where it names *Hunna Wood*
([ADR-0219](adr/0219-test-games-in-place-of-hunna-wood.md)).

- **The very modest laptop** (M6d.0c): its inventory, given on 2026-11-06 from `lshw -short` and
  `inxi -CGmSz`, lacks its display's size and rate, its storage and its Vulkan driver, which
  `nix-shell -p inxi pciutils vulkan-tools --run "inxi -GDz; vulkaninfo --summary"` and
  `niri msg outputs` give; then its measures.
- **The music player's Android prototype** (M6d.0c): written in Compose and measured on the Pixel.
- **Checkpoint A, in the Windows virtual machine** (M6c.1a): the editor's window; flying through
  the zone, selecting, rearranging it and playing it; the raw motion, the cursor held, the game's
  child process and the system's firewall, the dialogs.
- **The system's dialog** (M6c.1b): opened on the owner's Linux desktop and in the Windows virtual
  machine.
- **The cursor held and the raw motion** (M6c.1b): on the owner's Linux desktop, X11 through
  XWayland and in the Windows virtual machine.
- **Flying and orbiting in the zone** (M6c.2a): on the owner's Linux desktop and X11 through
  XWayland.
- **Sixteen passes to a submission** (M6c.2a): on Direct3D 12, in the Windows virtual machine.
- **Picking in the zone** (M6c.2b): on the owner's Linux desktop and X11 through XWayland; Alt and a click
  with a mouse of a fine resolution, which must not read it as an orbit; the outline on Direct3D
  12, in the Windows virtual machine.
- **Two viewports and Blender's way** (M6c.2c): on the owner's Linux desktop and in the Windows
  virtual machine.
- **The handles** (M6c.3a): their feel, their hit areas and the snapping, in the yard and the zone,
  and in the Windows virtual machine; Escape during a drag, which tests reach through the model,
  not through a key of the window.
- **Creating and arranging** (M6c.3b): creating, duplicating, copying between the yard and the
  zone, grouping and nudging, on Linux and in the Windows virtual machine.
- **Placing prefabs and the history panel** (M6c.3c): in the yard and the zone, and in the Windows
  virtual machine.
- **Play** (M6c.4a): Play, Pause, Step and Stop with *Hunna Wood* and the yard, on Linux and in the
  Windows virtual machine.
- **Edits while playing** (M6c.4b): on Linux and in the Windows virtual machine.
- **Building the game again** (M6c.4c): a change of *Hunna Wood*'s rules built and started again,
  on Linux and in the Windows virtual machine; there, a build while the game runs, which Windows
  may keep from replacing a running program, and the console windows that must not open.
- **The hierarchy** (M6c.5a): on the zone; read by Orca on Linux and by Narrator in the Windows
  virtual machine.
- **Reparenting by drags** (M6c.5b): on the zone, and in the Windows virtual machine.
- **Hiding and locking** (M6c.5c): parts of the zone, and in the Windows virtual machine; a drop
  onto what is hidden or locked, end to end, which tests check through the casts alone.
- **The regions of tabs** (M6c.5d): resized and rearranged, on Linux and in the Windows virtual
  machine.
- **Layouts kept, floating and closed panels** (M6c.5e): a layout kept across starts, a panel
  floated, moved and sized by its bar and its corner, closed and opened again where it was, on
  Linux and in the Windows virtual machine.
- **A hidden game at rest** (M6c.Q2c): *Hunna Wood* minimized in the Windows virtual machine,
  drawing nothing and waking not at all, and drawing again once shown.
- **G20, in the Windows virtual machine.** A source asset whose `.meta` differs from its own name
  by case only, cooked twice: whether its ID changes at each cook, and whether `slime-cook watch`
  cooks it again without end (see [Left by the audit of 0.4](debt.md#left-by-the-audit-of-04)).

## Known debt

The known debt still open, 165 items, is in [`debt.md`](debt.md).
