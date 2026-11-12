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
- Done: M6d.0a to M6d.2b2b3: M6d.1, how an application is
  written, closed on 2026-11-10; M6d.2, typed styles, cut in two and its halves cut again, closed
  by [M6d.2b2b3](stage-6d.md#m6d2b2b3--tokens), tokens, on 2026-11-11
  ([ADR-0238](adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md)).
- M6d.3, documents in KDL, cut in three at its start, on 2026-11-11: every document of uwui is
  KDL, read by a reader of its own
  ([ADR-0241](adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)). Done:
  [M6d.3a](stage-6d.md#m6d3a--uwuis-reader-of-kdl), uwui's reader of KDL, on the same day; M6d.3b
  cut in two, [M6d.3b1](stage-6d.md#m6d3b1--themes-in-kdl), themes, on the same day, and
  [M6d.3b2](stage-6d.md#m6d3b2--style-sheets-in-kdl), style sheets, on 2026-11-12. M6d.3c cut in
  three on 2026-11-12: done, [M6d.3c1](stage-6d.md#m6d3c1--shortcuts-in-kdl), shortcuts, on the
  same day; in progress, [M6d.3c2](#m6d3c2--docks-layouts-in-kdl), docks' layouts; then
  [M6d.3c3](stage-6d.md#m6d3c3--reflection-out-of-uwui), reflection out.
- In parallel, the owner's choice of 2026-11-11: two tracks at most, each milestone in its own
  session.
  - Track A, uwui's core, in the main checkout: M6d.3a to M6d.3b2, then M6d.3c1 to M6d.3c3.
  - Track B, beside it, in a worktree: [M6d.6](stage-6d.md#m6d6--a-series-of-headless-compositors),
    the series of compositors, done on 2026-11-11
    ([ADR-0240](adr/0240-the-series-input-its-table-and-its-jobs.md)); then M6d.4, cut in three
    at its start, on 2026-11-11
    ([ADR-0242](adr/0242-the-canvas-moved-in-three-steps-tokens-read-when-drawn-later.md)): done,
    [M6d.4a](stage-6d.md#m6d4a--the-canvass-prototype-and-its-adr), the canvas's prototype, on
    2026-11-12 ([ADR-0243](adr/0243-the-canvas-reads-its-items-by-index-from-a-buffer-or-a-texture.md));
    M6d.4b1 cut in two on 2026-11-12
    ([ADR-0244](adr/0244-the-engine-lends-its-device-through-a-guarded-door.md)): in progress,
    [M6d.4b1a](#m6d4b1a--the-canvas-moved-into-uwui-render), the canvas moved; then
    [M6d.4b1b](stage-6d.md#m6d4b1b--the-canvas-on-wgpu), on wgpu, and
    [M6d.4b2](stage-6d.md#m6d4b2--the-canvass-layout-chosen).
  - M6d.5 waits for M6d.3 and M6d.4, which both touch `slime-app`; M6d.10 to M6d.12 run alone.
  - Timed measures run one track at a time; shared files are edited by one session at a time.
- Waiting for the owner: the deletion of the branches `m6d1b-bench` and `m6d4a-prototype`, the
  last kept while measurements link to its code; the mirror of the game's assets, a private repository, which `scripts/fetch-assets.sh`
  cannot reach, CI living on its cache; and the
  [checks by hand](#checks-by-hand-waiting-for-the-owner).
- After Stage 6d: uwui's stages, Stage 6e, the rest of Stage 6c ([`stage-6c.md`](stage-6c.md)),
  then Stage 7 ([`ROADMAP.md`](ROADMAP.md#stage-7--runtime-systems--08)).

## Status

| Milestone | Status |
|---|---|
| [M6d.0a to M6d.3b2, and C1](stage-6d.md#where-it-stands) | Done |
| [M6d.3c1 — Shortcuts in KDL](stage-6d.md#m6d3c1--shortcuts-in-kdl) | Done |
| [M6d.3c2 — Docks' layouts in KDL](#m6d3c2--docks-layouts-in-kdl) | In progress |
| [M6d.3c3 — Reflection out of uwui](stage-6d.md#m6d3c3--reflection-out-of-uwui) | Planned |
| [M6d.4a — The canvas's prototype and its ADR](stage-6d.md#m6d4a--the-canvass-prototype-and-its-adr) | Done |
| [M6d.4b1a — The canvas moved into uwui-render](#m6d4b1a--the-canvas-moved-into-uwui-render) | In progress |
| [M6d.4b1b — The canvas on wgpu](stage-6d.md#m6d4b1b--the-canvas-on-wgpu) | Planned |
| [M6d.4b2 — The canvas's layout chosen](stage-6d.md#m6d4b2--the-canvass-layout-chosen) | Planned |
| [M6d.5 — The host's API](stage-6d.md#m6d5--the-hosts-api) | Planned |
| [M6d.6 — A series of headless compositors](stage-6d.md#m6d6--a-series-of-headless-compositors) | Done |
| [M6d.7 — The desktop host: its window and its frames](stage-6d.md#m6d7--the-desktop-host-its-window-and-its-frames) | Planned |
| [M6d.8 — The desktop host: clipboard, input methods, accessibility](stage-6d.md#m6d8--the-desktop-host-clipboard-input-methods-accessibility) | Planned |
| [M6d.9a — One core for agents, and its MCP server](stage-6d.md#m6d9a--one-core-for-agents-and-its-mcp-server) | Planned |
| [M6d.9b — Actions declared once](stage-6d.md#m6d9b--actions-declared-once) | Planned |
| [M6d.10 — uwui designed as a project](stage-6d.md#m6d10--uwui-designed-as-a-project) | Planned |
| [M6d.11 — The move](stage-6d.md#m6d11--the-move) | Planned |
| [M6d.12 — The audit of Stage 6d](stage-6d.md#m6d12--the-audit-of-stage-6d) | Planned |

## M6d.3c2 — Docks' layouts in KDL

The second third of M6d.3c, its questions decided at its start, on 2026-11-12
([ADR-0241](adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)'s status). It
follows the rules of [every milestone of the stage](stage-6d.md#stage-6d--uwui-leaves-the-engine).

- **Goal.** A dock's layout in KDL, and the layouts that the editor saved migrated once.
- **Crates.** `uwui`, `uwui-kdl`, `slime-editor`, `slime-benches`.
- **Delivers.**
  - A layout written and read by hand in `uwui::dock`: `version 1`, `default "editor 1"`, then
    its tree, `Split …` at the top; a later version left as it is; read leniently within its
    version, a default strictly.
  - `uwui-kdl`'s printers of names, texts and numbers, an `f32` read back to its bits.
  - The editor's layout in `<program>.layout.kdl`, migrated once from the TOML file, read a last
    time by copies of the layout's types in the editor, never written again.
  - The editor's default and `Program::layout` in KDL; `CHANGELOG.md`'s breaks.
- **Done when.** The editor's default, a user's layout with floating and closed panels, and one
  written before them, kept as fixtures, read the same as before; random layouts read the same
  both ways, bit for bit; a layout migrated comes back under its default; a damaged file is set
  aside; CI green.
- **Measured.** Reading the editor's documents at its start, `ui.tree.look`; the cooker's compile
  time, at rest.

## M6d.4b1a — The canvas moved into uwui-render

The first half of M6d.4b1, cut at its start, on 2026-11-12
([ADR-0244](adr/0244-the-engine-lends-its-device-through-a-guarded-door.md)). It follows the rules
of [every milestone of the stage](stage-6d.md#stage-6d--uwui-leaves-the-engine).

- **Goal.** The canvas and the glyphs' atlas live in `uwui-render`, still on `slime-gfx`'s
  handles, and the engine draws them through `slime-app`; images and figures unchanged.
- **Crates.** `uwui-render`, `slime-render`, `slime-app`, `slime-devui`, `slime-editor`,
  `slime-benches`, the examples; `scripts/`.
- **Delivers.**
  - The canvas's types, shader and renderer, and its tests, moved into `uwui-render`; a canvas's
    images given when it is drawn, so that a canvas is data alone; `uwui-render` without
    `slime-render`.
  - `Renderer::draw_canvas`, the canvas's types and its shader gone from `slime-render`.
  - `slime-app`'s bridge, which draws a display list or a canvas over the engine's target with its
    images; every caller through it, in the form that M6d.4b1b keeps.
  - The probe of the canvas's cost moved to the benchmarks; `CHANGELOG.md`'s breaks.
- **Done when.** The canvas's tests pass in `uwui-render`; the editor, the dev overlay, *Hunna
  Wood*'s menus and the `interface` and `widgets` examples draw their images unchanged;
  `slime-render` holds no canvas; CI is green.
- **Measured.** `render.canvas` and `ui.draw.menu` before and after, at 3840 × 2160 on the
  RX 6900 XT, within the interface's 0.5 ms; CI's jobs.

## Ideas waiting for their milestone

From two outside reviews, kept by the owner on 2026-11-10; each joins its milestone at its start.

- Stage 7's planning: the very modest laptop measured on
  [ADR-0216](adr/0216-light-geometry-and-the-gpus-floor.md)'s common floor before the renderer
  stands on it; one memory budget for every load and renderer ([`debt.md`](debt.md));
  the simulation's parallel run tried on one case before its systems.
- uwui's first stage: a channel's capacity, and its messages told apart, last value, kept events,
  large changes in parts ([ADR-0211](adr/0211-the-executor-and-other-threads-through-channels.md)).
- Later: cells loaded under a budget before Stage 10a's formats; a view's durable state keyed
  apart from its path; themes' parts tried on hard components; few concepts to start with; early
  trials of mobile and web life cycles.

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
- **Shortcuts on macOS** (M6d.3c1): the editor's menus and sheet of shortcuts in the macOS virtual
  machine, `⇧⌘Z` and Apple's symbols; ⌘C, ⌘V and ⌘Z, Option and the arrows, and Option typing a
  character, in a field; Blender's `Alt+A`, whose logical key Option changes; Option alone, which
  opens the bar of menus.

## Known debt

The known debt still open, 176 items, is in [`debt.md`](debt.md).
