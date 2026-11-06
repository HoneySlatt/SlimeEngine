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
  [ADR-0197](adr/0197-uwuis-repository-public-from-now.md)).
- Done: none yet.
- Now: [M6d.0](#m6d0--the-figures-before).
- [uwui's repository](https://github.com/HoneySlatt/uwui) holds its direction, its quality bar and its open decisions; its code
  moves there at M6d.11.
- May be cut at their start: M6d.1 into its survey and its decisions; M6d.5, AccessKit's translation
  apart.
- Stage 6c, paused since 2026-11-06, M6c.Q2e its last milestone done: its plan is in
  [`stage-6c.md`](stage-6c.md). It resumes after uwui's 1.0, the editor moved onto it
  ([ADR-0190](adr/0190-uwuis-requirements.md)).
- After Stage 6d: uwui's stages, in its repository, to its 1.0
  ([`ROADMAP.md`](ROADMAP.md#uwuis-stages)); then the rest of Stage 6c; then Stage 7, runtime
  systems, to be released as 0.8 ([`ROADMAP.md`](ROADMAP.md#stage-7--runtime-systems--08)).

## Status

| Milestone | Status |
|---|---|
| [M6d.0 — The figures before](#m6d0--the-figures-before) | Planned |
| [M6d.1 — How an application is written](#m6d1--how-an-application-is-written) | Planned |
| [M6d.2 — Typed styles](#m6d2--typed-styles) | Planned |
| [M6d.3 — Documents with serde](#m6d3--documents-with-serde) | Planned |
| [M6d.4 — The canvas in uwui, on wgpu](#m6d4--the-canvas-in-uwui-on-wgpu) | Planned |
| [M6d.5 — The host's API](#m6d5--the-hosts-api) | Planned |
| [M6d.6 — A series of headless compositors](#m6d6--a-series-of-headless-compositors) | Planned |
| [M6d.7 — The desktop host: its window and its frames](#m6d7--the-desktop-host-its-window-and-its-frames) | Planned |
| [M6d.8 — The desktop host: clipboard, input methods, accessibility](#m6d8--the-desktop-host-clipboard-input-methods-accessibility) | Planned |
| [M6d.9 — Agents reach an application](#m6d9--agents-reach-an-application) | Planned |
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
  [`measurements.md`](measurements.md), each within its limit of 20 minutes;
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

- **Goal.** The stage starts from figures measured, and from screens of reference chosen before
  any work and never tuned for.
- **Crates.** None; `scripts/`; the four programs' repositories, read only.
- **Delivers.** The screens of reference, chosen with the owner and written in uwui's quality bar:
  gpui-kit's "Hello, World!", a screen of Slatty, one of the music player, and those the owner
  adds. The four programs measured with their present frameworks: their start, their frames while
  they scroll and animate, their memory, their power at rest, on the owner's machine and the
  owner's very modest laptop, named. [ADR-0180](adr/0180-one-canvas-and-the-frameworks-foundations.md)'s
  figures: what `Renderer::new` costs an application in start, memory and video memory against
  the canvas alone; what a program on uwui compiles, its crates, its build time and its size.
- **Done when.** Each figure is measured, or says why not; the screens are written down.
- **How to test.** The existing measures, pointed at each program.
- **Measured.** As delivered.
- **By hand.** The owner: the laptop's inventory and its measures; the music player's Android
  prototype in Compose, on the Pixel.

### M6d.1 — How an application is written

- **Goal.** The owner decides how an application on uwui is written, from prototypes measured on
  the screens of reference.
- **Crates.** None kept: the prototypes live on a branch, then are thrown away.
- **Delivers.** A survey of GPUI with gpui-kit, Iced, Slint, Dioxus, Flutter, Jetpack Compose and
  SwiftUI; signals, views that say they changed, and messages, each prototyped on the screens over
  uwui's tree; the format of layouts; Rust's code reloaded with its state kept; the interface's
  executor and its waker; the time of each frame that motion reads; an application started in one
  call; styles written in code beside the themes' files. Each question put to the owner, then
  recorded in an ADR, and uwui's open decisions updated.
- **Done when.** Every question is decided and recorded; the prototypes' figures are kept.
- **Measured.** For each model and screen: lines, concepts to learn, CPU and allocations of a
  frame.

### M6d.2 — Typed styles

- **Goal.** uwui's styles cascade as typed values, without the engine's reflection.
- **Crates.** `uwui`, `slime-editor`, `hunnawood-editor`.
- **Delivers.** A part of a style as a type of its own, each field optional, merged by the
  cascade; the themes' named values resolved by type; styles written in code as M6d.1 decides.
- **Done when.** A property test: any style written as before reads the same through the typed
  cascade; the look's image tests and counters are unchanged; CI is green.
- **How to test.** `cargo test -p uwui -p slime-editor`; `scripts/check.sh`.
- **Measured.** The editor's interface and its counters; a frame that styles again after a change
  of theme.

### M6d.3 — Documents with serde

- **Goal.** uwui reads and writes its documents with `serde`, and the files that users saved open
  as before.
- **Crates.** `uwui`, `slime-cook`, `slime-editor`, `slime-color`, `hunnawood`.
- **Delivers.** Themes, style sheets, shortcuts and docks' layouts read and written by `serde` and
  `toml`, behind a feature; without it, a theme written in code. Decided at its start: the cooked
  form of these documents, and `slime-color`'s home with its reflection.
- **Done when.** Every document of the repository, and layouts and shortcuts saved by the editor
  kept as fixtures, read the same as before; `cargo tree -p uwui` holds neither `slime-reflect`
  nor `slime-serialize`; CI is green.
- **How to test.** `cargo test -p uwui -p slime-cook -p slime-editor`.
- **Measured.** Reading the editor's documents at its start.

### M6d.4 — The canvas in uwui, on wgpu

- **Goal.** uwui draws itself with a wgpu device and a target, without the engine's renderer, on a
  path that WebGL2 takes.
- **Crates.** `uwui-render`, `slime-render`, `slime-gfx`, `slime-app`, `slime-devui`,
  `slime-editor`, `hunnawood`; `AGENTS.md`.
- **Delivers.** First, a prototype thrown away: items handed on as flat values of instances,
  against items kept in a texture that the shader reads by their index, each counted against
  WebGL2's limits for the drawing that uwui's first stage plans, and measured. Then the canvas in
  `uwui-render` on the layout chosen, taking a device, its queue and a target; the engine handing
  it its device and composing it over its views; image tests on wgpu's GL backend at WebGL2's
  limits, if the owner accepts wgpu's `gles` feature; AGENTS.md's rule on wgpu's types naming
  `uwui-render`.
- **Done when.** The editor, the dev overlay and *Hunna Wood*'s menus draw as before in their image
  tests; a test draws a tree of uwui with a bare wgpu device; CI is green.
- **How to test.** `cargo test --workspace`; `scripts/check.sh`.
- **Measured.** `render.canvas` and `ui.draw.menu`, before and after, at 3840 × 2160 on the
  RX 6900 XT, within the interface's 0.5 ms.

### M6d.5 — The host's API

- **Goal.** A host drives uwui through one explicit API, and the engine is its first host.
- **Crates.** `uwui`, `slime-app`, `slime-platform`.
- **Delivers.** As [ADR-0181](adr/0181-uwuis-scope.md) decides and M6d.1 shapes: the surface
  created, lost, made again and resized, with its scale; the pointer, multi-touch, the keyboard and
  gamepads as events; the bridge to input methods; the lifecycle; the clipboard; requests for
  frames, the time of a frame, and a waker from any thread; the translation of meaning into
  AccessKit's tree moved into uwui; `slime-app` a host through this API alone.
- **Done when.** `slime-app` reaches uwui through the API alone; the editor's and the game's
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
  ([ADR-0173](adr/0173-the-platforms-of-applications.md)).
- **Done when.** The example rests on every compositor of the series; its input to present is
  within 4.17 ms in a headless sway at 240 Hz; the check fails on a program made to pull
  `slime-render`; CI is green, macOS included.
- **How to test.** `cargo test -p uwui`; the series' job; `scripts/input-to-present.sh`.
- **Measured.** The host's own cost against a bare winit and wgpu window, in the same run: start,
  memory, the binary's size, rest, input to present. The applications' budgets get first figures,
  not judged on the example.
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

- **Goal.** An agent reads and drives an application on uwui, as it does a game.
- **Crates.** uwui's channel and its MCP server, new crates.
- **Delivers.** A channel whose protocol does not depend on its transport, so that Android and the
  web can carry it later; an MCP server; the engine's own channel unchanged.
- **Done when.** An agent drives the example in a headless Weston; CI is green.
- **How to test.** The server's session tests.
- **Measured.** An answer of the channel.

### M6d.10 — uwui designed as a project

- **Goal.** uwui's repository is ready to receive its code. Builds nothing.
- **Delivers.** Put to the owner, then recorded: uwui's first stage, its milestones and their order,
  the trials before the authoring API is built proposed; how its ADRs are numbered; the moved
  crates' names; how the engine follows uwui after the move; which repository holds the
  interface's budgets; what stands for the engine in uwui, its benchmarks and a menu driven by a
  gamepad; how the engine keeps a series of compositors for its platform layer. uwui's
  `AGENTS.md`, roadmap and plan updated.
- **Done when.** The owner has validated uwui's documents.

### M6d.11 — The move

- **Goal.** uwui's code lives in its repository, and the engine depends on it.
- **Delivers.** The history of uwui's files filtered from a clone by `git filter-repo`, following
  their former paths, and merged into uwui's repository, checked for personal data, authored by
  the owner's identity alone, its links to the engine's ADRs made absolute; the crates that leave
  with it, as M6d.10 decides; its CI on Linux, Windows and macOS, and its builds for the web,
  Android and iOS; the engine depending on it; uwui's files removed here, every document updated.
- **Done when.** Both repositories' CI are green; the engine's budgets are unchanged.
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
is run with `--new --save <a file of its own>`, so that the owner's save is left alone.

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

The known debt still open, 164 items, is in [`debt.md`](debt.md).
