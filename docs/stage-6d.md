# Stage 6d, uwui leaves the engine

This file holds Stage 6d's whole plan: its rules, and each milestone in full, done and planned.
[`PLAN.md`](PLAN.md) holds its status, the milestone in progress and the checks by hand. When the
stage ends, its summary goes to [`history.md`](history.md) and this file is removed; its text stays
in git.

## Where it stands

- Planned with the owner on 2026-11-06, released with 0.7. Its decisions:
  [ADR-0194](adr/0194-stage-6d-uwui-leaving-the-engine.md),
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
  [ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md),
  [ADR-0221](adr/0221-compiling-within-its-budgets.md),
  [ADR-0222](adr/0222-what-a-theme-is.md),
  [ADR-0223](adr/0223-styles-written-in-code-through-tokens.md),
  [ADR-0224](adr/0224-a-files-change-reaches-the-application-through-the-development-tool.md),
  [ADR-0225](adr/0225-layouts-written-in-kdl-reached-from-code-by-their-names.md),
  [ADR-0226](adr/0226-an-application-started-in-one-call-each-root-given-its-look.md),
  [ADR-0227](adr/0227-the-time-that-motion-reads.md),
  [ADR-0229](adr/0229-styles-typed-as-declarations.md),
  [ADR-0230](adr/0230-the-cores-roles-and-each-kinds-defaults.md),
  [ADR-0231](adr/0231-an-elements-own-style-written-in-code.md),
  [ADR-0232](adr/0232-transitions-read-the-derives-colours-and-share-what-they-show.md),
  [ADR-0237](adr/0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md) and
  [ADR-0238](adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md).
- Widened on 2026-11-07 by the market's decisions
  ([ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md#decision)): M6d.1d, M6d.2, M6d.3,
  M6d.4, M6d.5, M6d.7, M6d.10 and M6d.11 gain what those decisions need of them; M6d.9 is cut in
  two ([ADR-0218](adr/0218-one-core-for-agents-in-uwui.md)).
- Done: [M6d.0](#m6d0--the-figures-before), the figures before, cut in three at its start
  ([ADR-0198](adr/0198-screens-of-reference-and-the-rivals-measured.md)):
  [M6d.0a](#m6d0a--the-screens-and-the-owners-programs),
  [M6d.0b](#m6d0b--measured-from-outside-and-ten-hello-world) and
  [M6d.0c](#m6d0c--the-applications-of-the-market).
- Done: [M6d.1](#m6d1--how-an-application-is-written), cut in four at its start.
  - Done: [M6d.1a](#m6d1a--the-survey-and-the-protocol), and
    [M6d.1b](#m6d1b--how-state-changes-the-interface): signals chosen with live values
    ([ADR-0205](adr/0205-signals-change-uwuis-interface.md)), checked against Slatty's list
    ([ADR-0206](adr/0206-slatty-ported-again-the-daw-later.md)).
  - Done: [M6d.1c](#m6d1c--rusts-code-reloaded-and-the-executor), on signals, cut in three at its
    start ([ADR-0210](adr/0210-signals-named-for-a-reload.md)):
    [M6d.1c1](#m6d1c1--signals-named-and-a-types-state-migrated), signals named by their authors
    and kept through `serde`; [M6d.1c2](#m6d1c2--the-executor-and-other-threads), the interface's
    executor and other threads through channels
    ([ADR-0211](adr/0211-the-executor-and-other-threads-through-channels.md)); M6d.1c3, cut in two
    at its start ([ADR-0212](adr/0212-a-functions-body-reloaded-through-subsecond-and-dx.md)),
    [M6d.1c3a](#m6d1c3a--a-functions-body-reloaded-through-subsecond), a function's body reloaded
    through `subsecond` and `dx`, and [M6d.1c3b](#m6d1c3b--the-rivals-reloads-timed), the rivals'
    reloads timed beside it, which closed M6d.1c.
  - Done: [M6d.1d](#m6d1d--layouts-styles-and-the-host), cut in two at its start, on
    2026-11-09 ([ADR-0222](adr/0222-what-a-theme-is.md)): done,
    [M6d.1d1](#m6d1d1--themes-and-styles-written-in-code), themes and styles written in code
    ([ADR-0223](adr/0223-styles-written-in-code-through-tokens.md),
    [ADR-0224](adr/0224-a-files-change-reaches-the-application-through-the-development-tool.md));
    M6d.1d2 cut in two at its start, on the same day
    ([ADR-0225](adr/0225-layouts-written-in-kdl-reached-from-code-by-their-names.md)): done,
    [M6d.1d2a](#m6d1d2a--layouts-their-notation-and-their-reload), layouts, their notation and
    their reload; M6d.1d2b cut in two at its start, on the same day
    ([ADR-0226](adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)): done,
    [M6d.1d2b1](#m6d1d2b1--an-applications-start-and-its-look), an application's start and its
    look, and [M6d.1d2b2](#m6d1d2b2--the-time-that-motion-reads), the time that motion reads
    ([ADR-0227](adr/0227-the-time-that-motion-reads.md)), which closed M6d.1 on 2026-11-10.
- Done: M6d.2, typed styles, cut in two at its start, on 2026-11-10
  ([ADR-0229](adr/0229-styles-typed-as-declarations.md)):
  done, [M6d.2a](#m6d2a--the-typed-cascade), the typed cascade;
  [M6d.2b](#m6d2b--tokens-defaults-and-styles-in-code), tokens, defaults and styles in code, cut
  in two at its start, on the same day
  ([ADR-0230](adr/0230-the-cores-roles-and-each-kinds-defaults.md)): done,
  [M6d.2b1](#m6d2b1--the-roles-and-each-kinds-defaults), the roles and each kind's defaults;
  [M6d.2b2](#m6d2b2--styles-written-in-code), styles written in code, cut in two at its start, on
  the same day ([ADR-0231](adr/0231-an-elements-own-style-written-in-code.md)): done,
  [M6d.2b2a](#m6d2b2a--an-elements-own-style-written-in-code), an element's own style written in
  code; [M6d.2b2b](#m6d2b2b--tokens-and-the-users-layer), tokens and the user's layer, cut in
  three at its start, on 2026-11-11
  ([ADR-0232](adr/0232-transitions-read-the-derives-colours-and-share-what-they-show.md)): done,
  [M6d.2b2b1](#m6d2b2b1--transitions-and-the-frame-of-a-change-of-theme), transitions and the
  frame of a change of theme; done,
  [M6d.2b2b2](#m6d2b2b2--the-users-layer-over-what-code-writes), the user's layer over what code
  writes ([ADR-0237](adr/0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md));
  done, [M6d.2b2b3](#m6d2b2b3--tokens), tokens
  ([ADR-0238](adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md)), which closed M6d.2
  on 2026-11-11.
- Done between M6d.1c3b and M6d.1d: [C1](#c1--compiling-within-its-budgets), the engine's
  compiling within its budgets ([ADR-0221](adr/0221-compiling-within-its-budgets.md)).
- The bench's branch, `m6d1b-bench`, waits for the owner's go to be deleted: uwui's measurements
  link to its code. M6d.1c's prototypes go on from it, on `m6d1c-bench`.
- [uwui's repository](https://github.com/HoneySlatt/uwui) holds its direction, its quality bar and
  its open decisions; its code moves there at M6d.11.
- May be cut at its start: M6d.5, AccessKit's translation apart.
- Stage 6c, paused since 2026-11-06, M6c.Q2e its last milestone done: its plan is in
  [`stage-6c.md`](stage-6c.md). It resumes after Stage 6e, the editor moved onto uwui's 1.0
  ([ADR-0190](adr/0190-uwuis-requirements.md),
  [ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md)).
- After Stage 6d: uwui's stages, in its repository, to its 1.0
  ([`ROADMAP.md`](ROADMAP.md#uwuis-stages)); then Stage 6e, the market measured, which moves
  *Hunna Wood* out for `games/open-world` and builds the tools that measure the engine against its
  rivals ([ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md),
  [`ROADMAP.md`](ROADMAP.md#stage-6e--the-market-measured)); then the rest of Stage 6c; then
  Stage 7, runtime systems, to be released as 0.8
  ([`ROADMAP.md`](ROADMAP.md#stage-7--runtime-systems--08)).

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
day ([ADR-0212](adr/0212-a-functions-body-reloaded-through-subsecond-and-dx.md)). Done on
2026-11-07, with M6d.1c3b.

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
  Rust's code closed; M6d.1c marked done. Done on 2026-11-07: six rivals timed, Makepad's change
  never shown here, its cause not found;
  [ADR-0212](adr/0212-a-functions-body-reloaded-through-subsecond-and-dx.md) stands, none of the
  rivals timed reloading Rust faster than `dx` allows; the open decision closed in
  [uwui's roadmap](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#the-first-stage-the-foundations-and-four-programs).
- **Measured.** From a change saved to the first frame that shows it, the median of 21; the state
  that each keeps, measured as its process and its window kept, the rest read in uwui's survey. In
  [uwui's `docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#the-rivals-reloads-2026-11-07).

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
- **Result.** Done on 2026-11-08, CI green on Linux and Windows
  ([ADR-0221](adr/0221-compiling-within-its-budgets.md)):
  - Dependencies optimised at the first level in debug builds.
  - mold links in the Nix shell.
  - The examples' editor's Play builds with the workspace's features, one line of
    `examples/editor.rs` changed on the owner's choice.
  - A profile `shipping`, optimised across crates.
  - Every budget measurable today within, the game's change by about a tenth, with mold; three
    sets of features remain, `cargo build --workspace`, a game with its editor, and the examples
    with the tests, in
    [`debt.md`](debt.md) ([`measurements.md`](measurements.md#c1-after-ryzen-7-5800x)).

### M6d.1d — Layouts, styles and the host

The rest of how an application is written, and what M6d.5's host API takes from it. Cut in two at
its start, on 2026-11-09, in the order of the questions' dependencies
([ADR-0222](adr/0222-what-a-theme-is.md)): the structure that a theme may change decides what a
layout describes, and the look that an application names at its start is a theme.

### M6d.1d1 — Themes, and styles written in code

- **Goal.** What a theme is, and how an application writes its styles.
- **Crates.** None kept. The prototypes go on from M6d.1c's, on the branch `m6d1c-bench`.
- **Delivers.** What a theme is: its values, its rules and the components' structure, which a
  theme may change, so that uwui's official themes are crates of their own
  ([ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md)). Styles written in code
  beside the themes' files. A style's change reloaded while the application runs.
- **Done when.** Each question is decided and recorded; a theme that changes a component's
  structure is written in the decision's examples.
- **Measured.** A style's change seen, against the budget of 2 s.
- **Result.** Done on 2026-11-09, CI green on Linux and Windows:
  - A theme holds values, rules and each component's parts, written as data; the component keeps
    its behaviour, focus, keys and meaning; the example's switch is a box with a tick in the core's
    look and a pill with a thumb in Material 3 Expressive
    ([ADR-0222](adr/0222-what-a-theme-is.md)).
  - An element's own style written in code, with its states, naming the themes' values by tokens,
    under the user's layer ([ADR-0223](adr/0223-styles-written-in-code-through-tokens.md)).
  - `uwui dev` watches the files and sends them over its one connection, which relays `dx`'s
    patches
    ([ADR-0224](adr/0224-a-files-change-reaches-the-application-through-the-development-tool.md)).
  - A style sheet's change, the program's part, within the budget of 2 s by far, the state kept
    ([uwui's figures](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#a-style-sheets-change-seen-2026-11-09));
    the tool's whole, when it is built.
  - [uwui's survey](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md#9-themes-that-change-a-components-structure)
    reads what ten references let a theme change of a component's parts.

M6d.1d2 was cut in two at its start, on 2026-11-09, layouts first: a look holds parts, written in
their notation ([ADR-0225](adr/0225-layouts-written-in-kdl-reached-from-code-by-their-names.md)).

### M6d.1d2a — Layouts, their notation and their reload

- **Goal.** How an application writes its layouts, and how a layout's change reaches it.
- **Crates.** None kept. The prototypes go on from M6d.1d1's, on the branch `m6d1c-bench`, which
  takes `kdl` 6.7.1 on the owner's go.
- **Delivers.** The format of layouts and their reload, the same format describing a theme's parts
  ([ADR-0222](adr/0222-what-a-theme-is.md)); its files sent by `uwui dev` as a theme's are
  ([ADR-0224](adr/0224-a-files-change-reaches-the-application-through-the-development-tool.md)).
- **Done when.** Each question is decided and recorded; uwui's open decision on the format of
  layouts is closed.
- **Measured.** A layout's change seen, against the budget of 2 s.
- **Result.** Done on 2026-11-09, CI green
  ([ADR-0225](adr/0225-layouts-written-in-kdl-reached-from-code-by-their-names.md)):
  - A layout is a screen's structure as data: elements, names, classes, literal texts, slots, and
    the commands that its buttons run; no expression, no style.
  - Code binds a named element by its name, through methods that a macro generates and the
    compiler checks; a name keys its element and the scopes below it.
  - Layouts and a theme's parts are KDL 2.0, the engine's UI documents among them, read by a
    reader of uwui's own; the other documents stay TOML.
  - A layout's change, the program's part, within the budget of 2 s by far, the state kept and the
    view's function not run again
    ([uwui's figures](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#a-layouts-change-seen-2026-11-09)).
  - `kdl` 6.7.1 overflows its stack on some inputs: the prototype guards the cases found, not
    every input.

M6d.1d2b was cut in two at its start, on 2026-11-09, the start and the look first: the two halves
share no decision
([ADR-0226](adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)).

### M6d.1d2b1 — An application's start and its look

The first half of M6d.1d2b, cut in two at its start
([ADR-0226](adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)). It follows
the rules of [every milestone of the stage](#stage-6d--uwui-leaves-the-engine).

- **Goal.** How an application starts, and the look that each of its roots names.
- **Crates.** None kept.
- **Delivers.** Each of these decided:
  - an application started in one call, and where the call lives, the engine compiling no second
    desktop host beside `slime-platform`'s;
  - the look named at every way of starting: the one call, and a host that embeds uwui, as the
    engine, a Kotlin activity that gives a surface, or a web page that exists; none chosen in
    silence ([ADR-0215](adr/0215-the-engine-measured-against-the-market.md)), and ADR-0190's
    "the default" amended;
  - what a look is, as a theme of [ADR-0222](adr/0222-what-a-theme-is.md); how an application's
    sheets and parts stack over it; one macro for an application's files;
  - the system's preferences that a look follows, and on Linux the desktop's portal;
  - blank, named Plain, defined, usable and accessible from its first frame, its name told apart
    from the *blank window*;
  - M6d.5 updated for them.
- **Done when.** Each question is decided and recorded.
- **Measured.** Nothing.
- **Result.** Done on 2026-11-09, CI green
  ([ADR-0226](adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)):
  - An application in the core's look names one crate, a *facade*, which holds `run` and chooses
    the host by platform; an attribute writes each platform's entry; `run` never returns; the
    engine depends on neither the facade nor uwui's hosts, which a check of M6d.7 proves.
  - Every root is given its look and its target when it is made, never by default; a look is a
    value, from code or from texts that the host gives, replaced while the root runs, its state
    kept.
  - An application's themes, sheets and parts stack over its look in ADR-0223's order; its parts
    for a component replace the look's whole; one macro, `files!`, embeds every file in every
    build.
  - A look follows the system's preferences, forced colours and saving power among them, read by
    one crate of uwui's that the engine's platform layer calls too; less motion keeps brief fades.
  - Blank is *Plain*, a variant of the core's look in forced colours' roles; the blank window
    keeps its name.
  - What each platform gives of a frame's time, of input's and of its preferences, read for
    M6d.1d2b2, is in
    [uwui's survey](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md#10-the-systems-preferences).

### M6d.1d2b2 — The time that motion reads

The second half of M6d.1d2b
([ADR-0226](adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)). It follows
the rules of [every milestone of the stage](#stage-6d--uwui-leaves-the-engine).

- **Goal.** The time of each frame that motion reads, and what timers, the executor and input's
  events read.
- **Crates.** None kept.
- **Delivers.** Each of these decided, as the attack on M6d.1d2b's draft raised them: the time
  that a frame's motion reads, for each target; what follows a late frame, and the engine's real
  time that reached uwui uncapped against [ADR-0107](adr/0107-the-execution-model.md); what a
  root's clock drives; what timers and tasks read; how often motion asks for frames; input's
  events' times; `frame_time`, the time's types and the time of day; M6d.5 updated.
- **Done when.** Each question is decided and recorded.
- **Measured.** Nothing: whether each compositor gives presentation times, and how much frames'
  starts vary at 240 Hz, come at M6d.6 and M6d.7.
- **Result.** Done on 2026-11-10, CI green
  ([ADR-0227](adr/0227-the-time-that-motion-reads.md)):
  - Motion reads when its frame shows, one time for each window: given, predicted from past
    presentations, asked, ready under a variable refresh, on the display's grid, or simulated in
    tests; never back. The engine gives the frame's start until M6d.7.
  - Each input event carries a time in the host's clock, converted; winit gives none, so the
    desktop host stamps reception, debt measured at M6d.7. A fling starts at its input's time, a
    transition at its first frame.
  - One host clock that counts a suspend, on every platform but the web.
  - No cap after a late frame: motion placed by its formula; the engine's interface reads real
    time uncapped by decision, and the debt on its clock closes.
  - A root's clock is its time and speed at each frame, a game's interpolated between its steps;
    hover, press, focus and uwui's own timings stay on the host's clock; what runs during a pause
    is a root of its own.
  - Two kinds of deadline: what changes the drawing waits while its root does not show; a task's
    timer, and a task woken outside a frame, wake the host without a frame.
  - A motion asks for a frame when its drawing changes visibly, under a cap that divides the
    refresh, halved for lasting motion while saving power.
  - `frame_time` gives way to `now`, `shown_at` and `next_frame`; two types, `Instant` and
    `RootTime`; the civil time, its zone and offset through the timer at a granularity.
  - What each platform gives of X11's Present, Android's past presentations, Windows'
    presentation manager, saving power, clocks across a suspend and the time of day is in
    [uwui's survey](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md#more-of-a-frames-time-of-power-and-of-clocks).

M6d.1d2b2 closed M6d.1d2b, M6d.1d2, M6d.1d and M6d.1. Next, [M6d.2](#m6d2--typed-styles).

### M6d.2 — Typed styles

- **Goal.** uwui's styles cascade as typed values, without the engine's reflection.
- **Crates.** `uwui`, `slime-editor`, `hunnawood-editor`.
- **Delivers.** Styles typed per component, as typed declarations
  ([ADR-0229](adr/0229-styles-typed-as-declarations.md)), merged by the cascade; the themes' named
  values resolved by type; no colour hidden in a component's defaults, each taken from its theme;
  styles written in code as M6d.1 decides: an element's own style with its states, the themes'
  values named by tokens, under the user's layer
  ([ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md),
  [ADR-0223](adr/0223-styles-written-in-code-through-tokens.md)). The part of a style that a theme
  adds, and rules typed for each of a component's parts, move to uwui's first stage
  (ADR-0229).
- **Done when.** A property test: any style written as before reads the same through the typed
  cascade; a test finds no colour in the components' defaults; the look's image tests and counters
  are unchanged; CI is green.
- **Measured.** The editor's interface and its counters; a frame that styles again after a change
  of theme.

Cut in two at its start, on 2026-11-10, the cascade first: the tokens, the defaults and the styles
in code are changes of behaviour that the old cascade cannot check
([ADR-0229](adr/0229-styles-typed-as-declarations.md)).

### M6d.2a — The typed cascade

The first half of M6d.2, cut in two at its start, on 2026-11-10
([ADR-0229](adr/0229-styles-typed-as-declarations.md)). It follows the rules of
[every milestone of the stage](#stage-6d--uwui-leaves-the-engine).

- **Goal.** uwui's styles cascade as typed declarations; reflection stays only to read the
  documents, until M6d.3.
- **Crates.** `uwui`, a new proc-macro crate of uwui's, its name settled at M6d.10;
  `slime-benches`. The editor's and the game's checks run unchanged.
- **Delivers.**
  - A derive that generates, from `Style` and its nested types, nested enums of declarations; a
    rule's part as a list of them; the cascade applying them to a `Style`, a `match` each, its
    readers unchanged. An element's own style stays a whole `Style` until M6d.2b, its declarations
    those of its fields whose bits differ from the default's.
  - A rule sets a colour whole; a part that sets a channel alone is refused.
  - A table of what each kind of element reads; a rule that names a kind and sets what it does not
    read is refused; the look's `[scroll]`, `[list]` and `[split]` trimmed.
  - One equality, by the floats' bits, for an own style's difference and for the styles made that
    elements share: today two own styles that differ only by `-0.0` share one style.
  - `whole_colors` and the reflected cascade removed from `Ui`.
- **Done when.**
  - A property test: random rules and own styles over every leaf, `-0.0` and NaN among their
    values, compose the same through the reflected cascade and the typed one, compared by bits.
  - The styles of the `widgets` example's tree under the look, in every theme of colours and
    density, and of the editor's window on the yard, kept as fixtures that outlive the reflected
    cascade.
  - A test checks the table: a leaf changes a kind's drawing exactly where the table says that the
    kind reads it; tests of the two refusals.
  - The look's image tests unchanged; the counters unchanged, but the allocations and bytes of a
    frame that styles again, which fall and are recorded; CI green.
- **How to test.** `cargo test -p uwui -p slime-editor`;
  `cargo run -p slime-benches -- --counters`; `scripts/check.sh`.
- **Measured.** `ui.tree.editor`, `ui.tree.look` (a frame after the look's colours changed, the
  frames of the transition that it starts) and the editor's counters, before and after, on the
  owner's machine at rest.
- **Result.** Done on 2026-11-10, CI green
  ([ADR-0229](adr/0229-styles-typed-as-declarations.md)):
  - A rule is a list of typed declarations, which `uwui-macros`'s derive generates from `Style`'s
    types; the cascade applies them, and no reflection runs when an element is styled. Reflection
    still reads the documents, until M6d.3.
  - A rule sets a colour whole. A rule for a kind sets only what that kind reads, after a table
    that a test checks against every kind's drawing; eleven rules of the look that set more were
    trimmed, no image changed.
  - Own styles are shared by their bits: two that differed by a signed zero shared one style.
  - A property test of 3 000 random cases, and two fixtures recorded through the reflected
    cascade, the `widgets` example's 1 212 elements in six looks and the editor's window's 103,
    read the same through the typed cascade.
  - Under the editor's look, the frame that styles every element again takes 27 % less time and
    allocates 7 % less; reading the look's documents allocates 7 % more until M6d.3, debt that the
    owner accepted ([measurements](measurements.md#m6d2a-the-typed-cascade)).
  - The part of a style that a theme adds waits for uwui's first stage, where a theme reads it.

### M6d.2b — Tokens, defaults and styles in code

The second half of M6d.2 ([ADR-0229](adr/0229-styles-typed-as-declarations.md)).

- **Goal.** A component's colours taken from its theme by typed tokens, and styles written in
  code as [ADR-0223](adr/0223-styles-written-in-code-through-tokens.md) decides.
- **Crates.** `uwui`, `slime-editor`, `slime-devui`, `hunnawood-editor`, the examples.
- **Done when.** Tests: a state set in code beats a rule's state; a field set in code to its
  default beats a rule; the size of text reaches every text but icons, and a list's rows; a
  missing token warns once; the call's place is kept; no colour in
  the components' defaults, each kind drawn on an empty theme and each colour traced to a token;
  the look's image tests and counters unchanged; CI green.
- **Measured.** The editor's interface and its counters; `ui.tree.look`'s frame after a change of
  theme and its transition.

Cut in two at its start, on 2026-11-10, the theme's side first: the styles written in code read
what the roles and the defaults decide
([ADR-0230](adr/0230-the-cores-roles-and-each-kinds-defaults.md)).

### M6d.2b1 — The roles and each kind's defaults

The first half of M6d.2b, cut in two at its start, on 2026-11-10
([ADR-0230](adr/0230-the-cores-roles-and-each-kinds-defaults.md)). It follows the rules of
[every milestone of the stage](#stage-6d--uwui-leaves-the-engine).

- **Goal.** No colour hidden in a component's defaults: each kind's colours taken from its look
  through the core's roles.
- **Crates.** `uwui`, `slime-benches`, the examples. The editor's and the game's checks run
  unchanged.
- **Delivers.**
  - The core's seven roles and their fallbacks, read once as colours when the interface takes its
    themes; a role's value of another type refuses the themes; a role that the themes lack takes
    its fallback, with one warning for each role and interface, given only when themes are given.
  - Each kind's base style from the roles, in one table with what each kind reads;
    `Style::default()` without a colour but transparent; the styles made shared by kind too.
  - The look's themes and the `interface` example's giving each role.
  - The bench's own style that writes white text written again, since it would now win over the
    rules.
- **Done when.**
  - A test draws every kind, in each configuration of the test of what each kind reads, under a
    theme that gives each role a colour of its own: every colour drawn is a role's, transparent,
    or a literal that the test names; every colour that a kind reads draws its role.
  - Tests: an interface given no theme draws in the fallbacks; a role's value of another type is
    refused, by its theme and name; a role lacking takes its fallback and is warned of once.
  - The look's image tests unchanged; the two fixtures of styles recorded again where a kind's base
    changed a leaf that it does not draw, the images proving that nothing drawn changed; the
    counters unchanged but those of reading the look's documents, recorded again; CI green.
- **How to test.** `cargo test -p uwui --features editor-look`; `cargo test -p slime-examples
  --test widgets --test editor`; `cargo run -p slime-benches -- --counters`; `scripts/check.sh`.
- **Measured.** `ui.tree.editor`, `ui.tree.look` (a frame after the look's colours changed, the
  frames of the transition that it starts) and the editor's counters, before and after, on the
  owner's machine at rest.
- **Result.** Done on 2026-11-10, CI green
  ([ADR-0230](adr/0230-the-cores-roles-and-each-kinds-defaults.md)):
  - The core declares seven roles, read once as colours when the interface takes its themes. A
    role's value of another type, or a colour with a channel that is no finite number, refuses
    the themes; a role lacking takes its fallback, today's colour, with one warning.
  - Each kind starts from a base made from the roles, in one table with what the kind reads;
    `Style::default()` holds no colour but transparent; the styles made are shared by base too.
  - A test draws every kind, in each configuration of the test of what each kind reads, under a
    theme that gives each role a colour of its own: every colour drawn is a role's, transparent,
    or a literal that it names, the picker's content and markers and an image untinted; every
    colour that a kind reads is drawn in its role.
  - The images are unchanged. The fixtures of the styles were recorded again: 1 195 of their
    1 315 elements changed, only on colours that their kind does not draw.
  - No frame changed beyond the runs' spread. Reading the look's documents allocates 4 % more,
    and the frame of the change 288 bytes more, the card having expected only the first; the
    machine was not at rest during the measures
    ([measurements](measurements.md#m6d2b1-the-roles-and-each-kinds-defaults)).
  - Code that writes a colour transparent now takes its kind's role even where no rule selects
    the element: a known debt widened, followed at M6d.2b2 ([`debt.md`](debt.md)).

### M6d.2b2 — Styles written in code

The second half of M6d.2b ([ADR-0230](adr/0230-the-cores-roles-and-each-kinds-defaults.md)).

- **Goal.** Styles written in code, with their states and the themes' values named by tokens,
  under the user's layer, as [ADR-0223](adr/0223-styles-written-in-code-through-tokens.md)
  decides.
- **Crates.** `uwui`, `uwui-macros`, `slime-editor`, `slime-devui`, `slime-app`,
  `slime-benches`, `hunnawood`, the examples.
- **Delivers.** Decided at its start, with the owner: a declaration's payload holding at each
  colour and length whether it is a literal or a token, and whether it records the state that set
  it, which [ADR-0227](adr/0227-the-time-that-motion-reads.md)'s two clocks read; the token as a
  public type, and an application's own tokens and their lists; the derive's list of colours and
  lengths, which transitions then read in place of their list kept by hand, and the user's layer
  and forced colours later; styles written in code with their states and ADR-0223's order, the
  call's place kept; the user's layer: the size of text, the density's factor on the lengths that
  code writes, the colours that code writes giving way under high contrast; the debts that code
  cannot set a field to its default against a rule, and that an own style's arrays, lists and
  options replace the rules' whole; a field of text named by a theme; which of the 179
  `Style::default()` calls and 222 assignments of a style's field outside uwui move, and when;
  whether a style's change in code is timed through `subsecond`, which the workspace lacks.
- **Done when.** Tests: a state set in code beats a rule's state; a field set in code to its
  default beats a rule; the size of text reaches every text but icons, and a list's rows; a token
  that the themes lack warns once; the call's place is kept;
  transitions read the derive's list and move every colour that a style holds; the look's image
  tests unchanged, and its counters but those that fall; CI green.
- **Measured.** The editor's interface and its counters; `ui.tree.look`'s frame after a change of
  theme and its transition.

Cut in two at its start, on 2026-11-10, behaviour first, each abstraction with its first reader
([ADR-0231](adr/0231-an-elements-own-style-written-in-code.md)): the tokens and the payload have
no reader until the user's layer, which tells a literal written in code from a token.

### M6d.2b2a — An element's own style written in code

The first half of M6d.2b2, cut in two at its start, on 2026-11-10
([ADR-0231](adr/0231-an-elements-own-style-written-in-code.md)). It follows the rules of
[every milestone of the stage](#stage-6d--uwui-leaves-the-engine).

- **Goal.** An element's own style written in code, with its states, under the order of
  ADR-0231: a rule's state over a base set in code, the code's states over the rules'.
- **Crates.** `uwui`, `uwui-macros`, `uwui-render`, `slime-editor`, `slime-devui`, `slime-app`,
  `slime-benches`, `hunnawood`, the examples.
- **Delivers.**
  - The derive numbers a style's leaves, an array one for each element, and says which lay out.
  - An own style as the declarations that code wrote, with their states, built by
    `Element::style(|s| ...)`, which keeps its call's place; a hover or a press without the fields
    that lay out; a debug warning, once, for a hover or a press written for a kind that the
    pointer never reaches.
  - The constructors without a style, `Ui::add_root` and `Ui::open_layer` taking the builder's
    function; every call site of the repository moved, writing only what it wrote apart from the
    default before.
  - The cascade's mask order; the styles made shared by the own states that hold too.
  - The look's keyboard focus shown on a list's rows, a row's marks, links and a notification's
    close button, in a commit of its own.
- **Done when.**
  - Tests: a state set in code beats a rule's state; a rule's state beats a base set in code; a
    base set in code beats a stateless rule that follows a stateful one; a field set in code to its
    default beats a rule; a width set in code keeps the rule's height; an own hover shows where no
    rule names `Hovered`, and goes; a hover's builder offers no field that lays out (a doc test that
    fails to compile); the call's place read back against `file!()` and `line!()`; every leaf
    through the mask order; the property test's rules against reflection, as before.
  - The look's image tests and the two fixtures of styles unchanged; the counters unchanged but
    those recorded again, each with its reason; a counter of a frame that builds again; CI green.
  - A test draws each of the four buttons focused by the keyboard and finds `$focus`.
- **How to test.** `cargo test -p uwui --features editor-look`; `cargo test -p slime-examples
  --test widgets --test editor`; `cargo run -p slime-benches -- --counters`; `scripts/check.sh`.
- **Measured.** `ui.tree.editor`, `ui.tree.look` and the editor's counters, before and after, on
  the owner's machine at rest; `uwui`'s compile times, its derive grown
  (`scripts/compile-times.sh`).
- **Result.** Done on 2026-11-10, CI green
  ([ADR-0231](adr/0231-an-elements-own-style-written-in-code.md)):
  - An element's code writes its own style, `Element::style(|s| ...)`, with its states; what it
    sets is kept whatever its value, an array by element. The place of each call is kept, which
    M6d.9a gives to agents.
  - The cascade takes the mask order: the rules in their order, then the own base where the last
    rule that set a field names no state, then the own states that hold. A random test checks it
    against a model; a test runs every one of the 66 leaves through it. A hover or a press written
    in code offers no setter that lays out.
  - The constructors lost their `Style`; every call site of the repository moved, writing only
    what it wrote apart from the default before: the fixtures of the styles and the images are
    unchanged.
  - The look shows the keyboard's focus on rows, marks, links and a notification's close.
  - A frame that builds again allocates 12 % more, each element's declarations in a list of its
    own; no frame's time changed beyond the runs' spread, the machine not quite at rest; the times
    of compiling were not measured, other sessions building
    ([measurements](measurements.md#m6d2b2a-an-elements-own-style-written-in-code)).
  - A change of a control's size, which a state may now write, lays the element out again.

### M6d.2b2b — Tokens and the user's layer

The second half of M6d.2b2 ([ADR-0231](adr/0231-an-elements-own-style-written-in-code.md)).

- **Goal.** The themes' values named in code by tokens, and the user's layer over what code
  writes, as [ADR-0223](adr/0223-styles-written-in-code-through-tokens.md) decides.
- **Crates.** `uwui`, `uwui-macros`, `slime-benches`, the examples.
- **Done when.** Tests: the size of text reaches every text but icons, and a list's rows
  ([ADR-0237](adr/0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md));
  a token that the themes lack warns once; a token's value of another
  type refuses the themes; transitions read the derive's list and move every colour that a style
  holds; the look's image tests unchanged, and its counters but those that fall; CI green.
- **Measured.** The editor's interface and its counters; `ui.tree.look`'s frame after a change of
  theme and its transition.

Cut in three at its start, on 2026-11-11, each abstraction with its first reader
([ADR-0232](adr/0232-transitions-read-the-derives-colours-and-share-what-they-show.md)):
transitions and the frame of a change of theme, which no image test sees, apart; then the user's
layer, in which everything that code writes is a literal; then tokens, which the user's layer tells
from a literal.

### M6d.2b2b1 — Transitions and the frame of a change of theme

The first third of M6d.2b2b, cut in three at its start, on 2026-11-11
([ADR-0232](adr/0232-transitions-read-the-derives-colours-and-share-what-they-show.md)). It
follows the rules of [every milestone of the stage](#stage-6d--uwui-leaves-the-engine).

- **Goal.** Transitions move every colour that a style holds, from the derive's list, and the frame
  after a change of theme and its transition stay within targets of allocations.
- **Crates.** `uwui`, `uwui-macros`, `slime-benches`.
- **Delivers.**
  - The derive lists a style's colours, from the fields marked `#[declare(color)]`; a colour
    unmarked fails to compile.
  - Transitions read that list in place of their own; the backdrop of a layer's root joins it.
  - Elements that move alike, from the same colours towards the same style since the same time,
    share one style shown, made once at each frame; an element styled again leaves them.
  - Drawing again a text that the cache of texts holds copies neither its style nor the cache's
    key.
  - `ui.tree.look` counts a frame of the transition; CI bounds both frames.
- **Done when.**
  - Tests: a transition moves every colour of the derive's list, the backdrop and a shadow's
    among them; elements that move alike show one style; an element styled again during its
    transition ends on its own target while the others end on theirs.
  - The targets of ADR-0232 met for the frame of the change and for the first frame of its
    transition, checked by `--counters`.
  - The look's image tests and the two fixtures of styles unchanged; the counters unchanged but
    those recorded again, each with its reason; CI green.
- **How to test.** `cargo test -p uwui --features editor-look`; `cargo test -p slime-examples
  --test widgets --test editor`; `cargo run -p slime-benches -- --counters`; `scripts/check.sh`.
- **Measured.** `ui.tree.look` and `ui.tree.editor`, before and after, on the owner's machine at
  rest, with the owner's go.
- **Result.** Done on 2026-11-11, CI green
  ([ADR-0232](adr/0232-transitions-read-the-derives-colours-and-share-what-they-show.md)):
  - Transitions move every colour that a style holds, from the derive's list; a field that names
    `LinearColor` without its mark fails to compile. A modal layer's backdrop now fades with a
    change of theme.
  - Elements that move alike share one style shown, made once at each frame; three tests show that
    elements that differ by their target, by the colours they started from or by their start share
    nothing.
  - The cache of texts finds a text without copying its key, and paint lays a label's text out
    from its node.
  - Under the editor's look, the frame after a change of colours allocates 8.8 times for each
    element styled again, against 31.5 and a target of 20; the first frame of its transition 7.8
    for each element drawn, against 30.5 and a target of 19, which `--counters` checks
    ([measurements](measurements.md#m6d2b2b1-transitions-and-the-frame-of-a-change-of-theme)).
  - The images and the two fixtures of styles are unchanged. The frames' times were not measured:
    the owner's machine could not be at rest.
  - What drawing again still allocates is debt, followed at uwui's first stage
    ([`debt.md`](debt.md)).

### M6d.2b2b2 — The user's layer over what code writes

The second third of M6d.2b2b
([ADR-0232](adr/0232-transitions-read-the-derives-colours-and-share-what-they-show.md)), its
questions decided at its start, on 2026-11-11
([ADR-0237](adr/0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md)). It
follows the rules of [every milestone of the stage](#stage-6d--uwui-leaves-the-engine).

- **Goal.** The size of text that the user chose reaches every text but icons, and a list's rows;
  the user's preferences are given as one value.
- **Crates.** `uwui`, `slime-benches`, the examples; `slime-editor`, which gives less motion.
- **Delivers.**
  - `Preferences` in uwui's core, the size of text and less motion, given by
    `Ui::set_preferences`, which replaces `Ui::set_reduced_motion`; every call site moved.
  - `TextSize`, a curve of sorted stops, linear by default, `TextSize::factor`; a stop or factor
    not finite, a ratio outside 0.5 to 3, or stops out of order refused.
  - The size of text applied in the cascade's fourth step to every text's size, the rules' and the
    code's, but a text whose first font is its style's font of icons; the styles made again when
    it changes.
  - A list's rows grown by the curve's ratio at the list's size of text, whole in pixels of the
    screen, read through one accessor.
  - uwui's and the `widgets` example's boxes of fixed width that hold a text made least widths.
  - `ui.tree.look` counts the frame after a change of the size of text; CI bounds it by the same
    tree's first frame at that size, for each element.
  - Density and more contrast leave what code writes as numbers: nothing built.
- **Done when.**
  - Tests: the size of text reaches a text sized by a rule or by code, not an icon; a curve
    interpolates its stops and refuses what it must; a list's rows grow and never overlap, at
    scales 1.25 and 2 and at 200 % text; less motion through `Preferences` ends transitions; a
    colour and a length written in code stay under high contrast and the compact theme.
  - At 200 %, no text of the `widgets` example, its layers open, cut by its box or by any ancestor
    up to the first that scrolls.
  - The images and the editor's fixture of styles unchanged, the `widgets` example's recorded
    again where it writes least widths; the other counters unchanged; CI green.
- **How to test.** `cargo test -p uwui --features editor-look`; `cargo test -p slime-examples
  --test widgets --test editor`; `cargo run -p slime-benches -- --counters`; `scripts/check.sh`.
- **Measured.** `ui.tree.look` and `ui.tree.editor`, and the editor's counters, before and after,
  on the owner's machine at rest, with the owner's go.
- **Found before its start:** in [ADR-0237](adr/0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md#context).
- **Result.** Done on 2026-11-11, CI green
  ([ADR-0237](adr/0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md)):
  - A program gives the user's size of text and less motion as one value,
    `Ui::set_preferences(Preferences { .. })`, in place of `Ui::set_reduced_motion`.
  - The size of text, a curve of stops or a factor, scales every text's size in the cascade's last
    step, whoever wrote it, but icons'; a list's rows grow with it, their height whole in pixels of
    the screen, read in one place, so that rows no longer overlap at a scale that is not whole.
  - Density and high contrast leave what code writes as numbers; a test checks it under the look's
    compact and high-contrast themes.
  - At 200 %, no label, button, toggle, segment, tab or check box of the `widgets` example is cut;
    uwui's sheet of shortcuts, the example's labels and values and its statistics were the five
    that were, and a field's, a menu item's or a title's text is not checked
    ([`debt.md`](debt.md)).
  - The frame after a change of the size of text allocates no more for each element than the same
    tree's first frame at that size, which CI checks; its times were not measured, the machine not
    at rest ([measurements](measurements.md#m6d2b2b2-the-users-layer-over-what-code-writes)).
  - The images are unchanged; the `widgets` example's fixture of styles was recorded again for its
    least widths alone.

### M6d.2b2b3 — Tokens

The last third of M6d.2b2b
([ADR-0232](adr/0232-transitions-read-the-derives-colours-and-share-what-they-show.md)), its
questions decided at its start, on 2026-11-11
([ADR-0238](adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md)). It follows the rules
of [every milestone of the stage](#stage-6d--uwui-leaves-the-engine).

- **Goal.** The themes' values named in code by tokens of four kinds, declared by each crate.
- **Crates.** `uwui`, `slime-benches`, the examples.
- **Delivers.**
  - `Token<K>` of the kinds `Color`, `Length`, `Duration` and `Number`, its fallback a value, an
    sRGB colour or another token; a crate's list, `Tokens`; `Ui::declare_tokens`, which refuses
    a name declared again with another kind or fallback.
  - The core's seven roles as public tokens, in place of `ColorRole`.
  - `set_style` reads each declared token once: a value of another kind, not finite, or a negative
    duration refuses the themes; a token lacking takes its fallback with one warning, only when
    themes are given; a token written but not declared, the same.
  - The own style's setters take a literal or a token of their kind; two own styles are the same
    only if they name the same tokens.
  - The `interface` example declares and writes its own tokens.
  - A counter of a tree whose own styles name tokens.
- **Done when.**
  - Tests of each refusal and warning above; a token follows the look's compact and high-contrast
    themes where a number written in code stays; a duration in a padding fails to compile; a token
    and a literal of the same value share no style.
  - The images and the two fixtures of styles unchanged; the counters unchanged but the new one;
    CI green.
- **How to test.** `cargo test -p uwui --features editor-look`; `cargo test -p slime-examples
  --test widgets --test editor`; `cargo run -p slime-benches -- --counters`; `scripts/check.sh`;
  the `interface` example drawn into an image on lavapipe, before and after.
- **Measured.** `ui.tree.look`, `ui.tree.editor` and the new counter; `uwui`'s compile times, its
  setters generic; on the owner's machine at rest, when track B builds nothing.
- **Found before its start:** in [ADR-0238](adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md#context).
- **Result.** Done on 2026-11-11, CI green
  ([ADR-0238](adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md)):
  - Code names a theme's value by a token of its kind, `s.padding(SPACE).background(FACE)`; a
    duration where a length goes does not compile. A crate lists its tokens and a program declares
    each list once; `set_style` reads each in its kind and refuses a value of another kind, a
    number that is not finite or a negative duration.
  - The core's seven roles are public tokens. A token that the themes lack, or that no list
    declared, takes its fallback, with one warning; the themes are read for the tokens declared
    alone. A cycle of fallbacks between `static` tokens is refused when its list is declared.
  - A token follows the look's compact and high-contrast themes through the names that a theme
    gives it, where a number written in code stays the author's. The `interface` example names its
    spaces by its own tokens; its image is unchanged.
  - A declaration of an own style keeps its 56 bytes, the place of its token held in the room
    that its state leaves: holding the token itself, the frame that builds the editor's inspector
    again allocated 1.5 % more bytes.
  - A tree whose own styles name tokens allocates 8.79 times for each element styled after a
    change of colours, within the target of 20. Two frames of `ui.tree.look` allocate 864 bytes
    more, each own style kept holding a list of its tokens, which the owner accepted; every other
    counter, the images and the two fixtures of styles are unchanged. The times were not measured,
    the machine not at rest, debt that `debt.md` follows
    ([measurements](measurements.md#m6d2b2b3-tokens)).
  - A theme's number that a rule names and that is not finite now refuses the themes too, the
    debt of the look's foundations paid where the milestone opened its code.
  - The look's own tokens, a text named by a theme and the tokens read by the canvas wait for
    uwui's first stage, M6d.3 and M6d.4 ([`debt.md`](debt.md)).

### M6d.3 — Documents with serde

- **Goal.** uwui reads and writes its documents with `serde`, and the files that users saved open
  as before.
- **Crates.** `uwui`, `slime-cook`, `slime-editor`, `slime-color`, `hunnawood`.
- **Delivers.** Themes, style sheets, shortcuts and docks' layouts read and written by `serde` and
  `toml`, behind a feature; without it, a theme's files read or turned into values another way,
  rules staying out of Rust's code ([ADR-0223](adr/0223-styles-written-in-code-through-tokens.md));
  a rule's declarations read through the `Deserialize` that their derive generates, unknown fields
  refused as reflection refuses them ([ADR-0229](adr/0229-styles-typed-as-declarations.md)). A primary modifier, Ctrl on Linux
  and Windows, Cmd on macOS, in the format of shortcuts, so that the shortcuts that users saved
  migrate once; a menu's shortcut shown as its platform writes it
  ([ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md),
  [ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md)). Decided at its start: the cooked
  form of these documents, layouts and a theme's parts among them, whose reader of KDL comes with
  uwui's first stage; whether a theme's values and rules are written in KDL too, so that a theme
  reads in one notation
  ([ADR-0225](adr/0225-layouts-written-in-kdl-reached-from-code-by-their-names.md)); a field of
  text named by a theme, `"$name"`, and how a theme's file writes a token's kind and unit
  ([ADR-0238](adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md)); and
  `slime-color`'s home with its reflection.
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
  naming `uwui-render`. What the canvas draws for a component, a knob or a meter, reads its
  tokens and less motion when it draws, so that a change of theme or of the preferences draws
  again without running a view
  ([ADR-0238](adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md)).
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
  item by item, against winit, Android, the web, UIKit and AppKit: a target for each root, input
  methods both ways, the insets, the system's preferences, an application's events, and a model of
  menus that macOS's menu bar takes
  ([ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md),
  [ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md)). Each root given its target, a
  window, a region of the host's image or a texture, and its look, which may be replaced while it
  runs; the system's preferences and their changes, from uwui's reader, which the engine's
  platform layer calls too; the facade, not the core, holding the one call, so that the engine
  depends on neither the facade nor uwui's hosts
  ([ADR-0226](adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)).
  The times ([ADR-0227](adr/0227-the-time-that-motion-reads.md)): for each window that presents,
  its frame's time and its source, its deadline where given, the display's rate and whether it
  varies; the next frame due and the next wake, a wake running tasks without drawing; each root's
  time and speed; each event's time and its source; the time spent suspended; the types `Instant`
  and `RootTime`; the civil time, its zone, its offset from UTC and their changes; a rate asked for
  a surface.
- **Done when.** `slime-app` reaches uwui through the API alone; a table in the API's ADR says, for
  each item, how it maps or why it needs no mapping on each of the five, on the engine, and on
  winit, split into Wayland, X11, Windows and macOS where it gives nothing; the editor's and the game's
  sessions, rest and input to present are unchanged; CI is green.
- **How to test.** `cargo test --workspace`; `scripts/check.sh`.
- **Measured.** Rest; input to present; the editor's start.

### M6d.6 — A series of headless compositors

- **Goal.** The editor and the game are checked on headless Weston, sway, KWin, Mutter and X11
  through Xvfb, and what differs is written down.
- **Crates.** `slime-mcp`; `scripts/`, `flake.nix`, `.github/workflows/`.
- **Delivers.** As [ADR-0195](adr/0195-a-series-of-headless-compositors.md) decides: a development
  shell and a job of CI of their own, run when the platform layer or the desktop host changes,
  every week and on demand; input for each family, the dependency of libei's client put to the
  owner; rest, an agent's session, decorations, the scale, frames and presentation times, how
  much frames' starts vary at 240 Hz ([ADR-0227](adr/0227-the-time-that-motion-reads.md)), the
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
  display that holds the window on Windows and X11, a display that reports none included; the
  time that the engine's windowed loop gives its interface. The platforms' times of frames read
  with code of the host's own, since winit gives none; input's events stamped at reception,
  measured, and a pull request proposed to winit for their times, after a search for an existing
  one; the notice that the system resumed, on each platform; whether the host reaches the
  presentation's times under wgpu: on Wayland, its own presentation feedback and commit timer on a
  surface whose commits Vulkan's driver makes, on Windows DXGI's statistics, the presentation
  manager and DWM's timing with several displays
  ([ADR-0227](adr/0227-the-time-that-motion-reads.md)). A waker from any thread; an example; a check that a program on uwui and its host compiles neither the
  renderer, the world nor the editor, and its other side, that the engine's programs compile none
  of uwui's hosts
  ([ADR-0226](adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)); a job of CI that builds and tests uwui and its host on macOS
  ([ADR-0173](adr/0173-the-platforms-of-applications.md)). A *blank window*, a program on winit and
  wgpu that only clears its window and presents it, among the programs measured
  ([ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md)). The host's crash reports,
  written on the machine and never sent
  ([ADR-0215](adr/0215-the-engine-measured-against-the-market.md)).
- **Done when.** The example rests on every compositor of the series; its input to present is
  within 4.17 ms in a headless sway at 240 Hz; the check fails on a program made to pull
  `slime-render`, and its other side on an engine's program made to pull uwui's host; a program
  on the host made to crash leaves its report on the machine; CI is green, macOS included.
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
  the engine's families: the world, play and editing. The place of the code that wrote an element's
  own style given to agents, its first reader
  ([ADR-0231](adr/0231-an-elements-own-style-written-in-code.md)).
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
  crates' names, and those of the facade and the reader of the system's preferences
  ([ADR-0226](adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)); how the engine follows uwui after the move; which repository holds the
  interface's budgets; what stands for the engine in uwui, its benchmarks and a menu driven by a
  gamepad; how the engine keeps a series of compositors for its platform layer; the measure that
  confirms games' interfaces on uwui's core with a layer of the engine's, or gives them a
  framework of their own if it fails
  ([ADR-0209](adr/0209-platforms-the-editor-and-agents.md)), as the engine's
  [`uwui-needs.md`](uwui-needs.md#for-m6d10) proposes it. What the engine, Opium, Pluto's client,
  Slatty, Magnolia and Aerith ask of uwui weighed for its first stage, each list's general needs apart;
  the colour model, the Android trial's milestone on a television box and the trial of content
  from outside placed among its milestones
  ([ADR-0233](adr/0233-uwuis-platforms-apples-chips-ios-and-the-tvs.md) to
  [ADR-0236](adr/0236-one-trial-of-content-from-outside-for-aerith-and-magnolia.md)). MyGo read for its contracts with the platforms and its
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
  Windows and macOS on Apple's chips, and its builds for the web, Android, iOS and tvOS
  ([ADR-0233](adr/0233-uwuis-platforms-apples-chips-ios-and-the-tvs.md)); the editor's look moved into the
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
