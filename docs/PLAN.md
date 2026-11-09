# Plan

This file says what is being worked on now: the current stage's status, the milestone in progress
and how it is checked, and the checks by hand that wait for the owner. A *stage* is a step of the roadmap; a *milestone* is a small, verifiable
step of a stage.

- The stage in progress, whole: [`stage-6d.md`](stage-6d.md).
- The known debt still open: [`debt.md`](debt.md).
- The finished stages and their releases: [`history.md`](history.md).
- Direction beyond this stage: [`ROADMAP.md`](ROADMAP.md).
- Decisions: [`adr/`](adr/README.md).

Every milestone ends with the definition of done in [`AGENTS.md`](../AGENTS.md#definition-of-done):
build, tests, clippy, rustfmt and rustdoc pass, the program was actually run, and the affected
documentation was updated and reread. Every milestone is also checked on Windows
([ADR-0014](adr/0014-amend-roadmap-for-stage-2-and-windows.md)).

## Current

- Stage 6d, uwui leaving the engine, released with 0.7: its whole plan in
  [`stage-6d.md`](stage-6d.md).
- Done: M6d.0a to C1, in the [status](#status) below. In progress:
  [M6d.1d1](#m6d1d1--themes-and-styles-written-in-code), the first half of M6d.1d.
- Waiting for the owner: the deletion of the branch `m6d1b-bench`, and the
  [checks by hand](#checks-by-hand-waiting-for-the-owner).
- After Stage 6d: uwui's stages, Stage 6e, the rest of Stage 6c ([`stage-6c.md`](stage-6c.md)),
  then Stage 7 ([`ROADMAP.md`](ROADMAP.md#stage-7--runtime-systems--08)).

## Status

| Milestone | Status |
|---|---|
| [M6d.0a — The screens and the owner's programs](stage-6d.md#m6d0a--the-screens-and-the-owners-programs) | Done |
| [M6d.0b — Measured from outside, and ten "Hello, World!"](stage-6d.md#m6d0b--measured-from-outside-and-ten-hello-world) | Done |
| [M6d.0c — The applications of the market](stage-6d.md#m6d0c--the-applications-of-the-market) | Done |
| [M6d.1a — The survey and the protocol](stage-6d.md#m6d1a--the-survey-and-the-protocol) | Done |
| [M6d.1b1 — The ideal code](stage-6d.md#m6d1b1--the-ideal-code) | Done |
| [M6d.1b2 — The bench, on the control](stage-6d.md#m6d1b2--the-bench-on-the-control) | Done |
| [M6d.1b3a — Three models on the bench](stage-6d.md#m6d1b3a--three-models-on-the-bench) | Done |
| [M6d.1b3b — The choice](stage-6d.md#m6d1b3b--the-choice) | Done |
| [M6d.1c1 — Signals named, and a type's state migrated](stage-6d.md#m6d1c1--signals-named-and-a-types-state-migrated) | Done |
| [M6d.1c2 — The executor, and other threads](stage-6d.md#m6d1c2--the-executor-and-other-threads) | Done |
| [M6d.1c3a — A function's body reloaded through subsecond](stage-6d.md#m6d1c3a--a-functions-body-reloaded-through-subsecond) | Done |
| [M6d.1c3b — The rivals' reloads, timed](stage-6d.md#m6d1c3b--the-rivals-reloads-timed) | Done |
| [C1 — Compiling within its budgets](stage-6d.md#c1--compiling-within-its-budgets) | Done |
| [M6d.1d1 — Themes, and styles written in code](#m6d1d1--themes-and-styles-written-in-code) | In progress |
| [M6d.1d2 — Layouts, an application's start and a frame's time](stage-6d.md#m6d1d2--layouts-an-applications-start-and-a-frames-time) | Planned |
| [M6d.2 — Typed styles](stage-6d.md#m6d2--typed-styles) | Planned |
| [M6d.3 — Documents with serde](stage-6d.md#m6d3--documents-with-serde) | Planned |
| [M6d.4 — The canvas in uwui, on wgpu](stage-6d.md#m6d4--the-canvas-in-uwui-on-wgpu) | Planned |
| [M6d.5 — The host's API](stage-6d.md#m6d5--the-hosts-api) | Planned |
| [M6d.6 — A series of headless compositors](stage-6d.md#m6d6--a-series-of-headless-compositors) | Planned |
| [M6d.7 — The desktop host: its window and its frames](stage-6d.md#m6d7--the-desktop-host-its-window-and-its-frames) | Planned |
| [M6d.8 — The desktop host: clipboard, input methods, accessibility](stage-6d.md#m6d8--the-desktop-host-clipboard-input-methods-accessibility) | Planned |
| [M6d.9a — One core for agents, and its MCP server](stage-6d.md#m6d9a--one-core-for-agents-and-its-mcp-server) | Planned |
| [M6d.9b — Actions declared once](stage-6d.md#m6d9b--actions-declared-once) | Planned |
| [M6d.10 — uwui designed as a project](stage-6d.md#m6d10--uwui-designed-as-a-project) | Planned |
| [M6d.11 — The move](stage-6d.md#m6d11--the-move) | Planned |
| [M6d.12 — The audit of Stage 6d](stage-6d.md#m6d12--the-audit-of-stage-6d) | Planned |

## M6d.1d1 — Themes, and styles written in code

The first half of [M6d.1d](stage-6d.md#m6d1d--layouts-styles-and-the-host), cut in two at its
start ([ADR-0222](adr/0222-what-a-theme-is.md)). It follows the rules of
[every milestone of the stage](stage-6d.md#stage-6d--uwui-leaves-the-engine).

- **Goal.** What a theme is, and how an application writes its styles.
- **Crates.** None kept. The prototypes go on from M6d.1c's, on the branch `m6d1c-bench`.
- **Delivers.** What a theme is: its values, its rules and the components' structure, which a
  theme may change, so that uwui's official themes are crates of their own
  ([ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md)). Styles written in code
  beside the themes' files. A style's change reloaded while the application runs.
- **Done when.** Each question is decided and recorded; a theme that changes a component's
  structure is written in the decision's examples.
- **Measured.** A style's change seen, against the budget of 2 s.

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
- **The build for shipping on Windows** (C1): `cargo build --profile shipping -p hunnawood
  --no-default-features` in the Windows virtual machine, then the game run with `--new --save
  <a file of its own>`.

## Known debt

The known debt still open, 165 items, is in [`debt.md`](debt.md).
