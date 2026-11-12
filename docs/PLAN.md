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
- Done: M6d.0a to M6d.2b2b3, closing M6d.1, how an application is written, and M6d.2, typed
  styles ([ADR-0238](adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md)).
- Done: M6d.3, documents in KDL, cut in three at its start, on 2026-11-11, closed on 2026-11-12:
  every document of uwui is KDL, read by a reader of its own
  ([ADR-0241](adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)):
  [M6d.3a](stage-6d.md#m6d3a--uwuis-reader-of-kdl), uwui's reader of KDL; M6d.3b cut in two,
  [M6d.3b1](stage-6d.md#m6d3b1--themes-in-kdl), themes, and
  [M6d.3b2](stage-6d.md#m6d3b2--style-sheets-in-kdl), style sheets. M6d.3c cut in
  three on 2026-11-12, [M6d.3c1](stage-6d.md#m6d3c1--shortcuts-in-kdl), shortcuts,
  [M6d.3c2](stage-6d.md#m6d3c2--docks-layouts-in-kdl), docks' layouts, and
  [M6d.3c3](stage-6d.md#m6d3c3--reflection-out-of-uwui), reflection out.
- In parallel, the owner's choice of 2026-11-11: two tracks at most, each milestone in its own
  session.
  - Track A, uwui's core, in the main checkout: M6d.3a to M6d.3c3, done; then M6d.9a, cut in
    five on 2026-11-12
    ([ADR-0246](adr/0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md)):
    in progress, [M6d.9a1a](#m6d9a1a--the-new-wire-under-todays-meaning).
  - Track B, in a worktree: [M6d.6](stage-6d.md#m6d6--a-series-of-headless-compositors),
    done; then [M6d.4](stage-6d.md#m6d4--the-canvas-in-uwui-on-wgpu), the canvas in uwui on
    wgpu, closed on 2026-11-12: [M6d.4a](stage-6d.md#m6d4a--the-canvass-prototype-and-its-adr),
    [M6d.4b1a](stage-6d.md#m6d4b1a--the-canvas-moved-into-uwui-render),
    [M6d.4b1b](stage-6d.md#m6d4b1b--the-canvas-on-wgpu) and
    [M6d.4b2](stage-6d.md#m6d4b2--the-canvass-layout-chosen), its layout chosen.
  - M6d.5 may start, M6d.3 and M6d.4 done; M6d.10 to M6d.12 run alone.
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
| [M6d.0a to M6d.3c3, and C1](stage-6d.md#where-it-stands) | Done |
| [M6d.4a — The canvas's prototype and its ADR](stage-6d.md#m6d4a--the-canvass-prototype-and-its-adr) | Done |
| [M6d.4b1a — The canvas moved into uwui-render](stage-6d.md#m6d4b1a--the-canvas-moved-into-uwui-render) | Done |
| [M6d.4b1b — The canvas on wgpu](stage-6d.md#m6d4b1b--the-canvas-on-wgpu) | Done |
| [M6d.4b2 — The canvas's layout chosen](stage-6d.md#m6d4b2--the-canvass-layout-chosen) | Done |
| [M6d.5 — The host's API](stage-6d.md#m6d5--the-hosts-api) | Planned |
| [M6d.6 — A series of headless compositors](stage-6d.md#m6d6--a-series-of-headless-compositors) | Done |
| [M6d.7 — The desktop host: its window and its frames](stage-6d.md#m6d7--the-desktop-host-its-window-and-its-frames) | Planned |
| [M6d.8 — The desktop host: clipboard, input methods, accessibility](stage-6d.md#m6d8--the-desktop-host-clipboard-input-methods-accessibility) | Planned |
| [M6d.9a1a — The new wire, under today's meaning](#m6d9a1a--the-new-wire-under-todays-meaning) | In progress |
| [M6d.9a1b to M6d.9a3b — The rest of M6d.9a](stage-6d.md#m6d9a1b--requests-in-flight-and-events) | Planned |
| [M6d.9b — Actions declared once](stage-6d.md#m6d9b--actions-declared-once) | Planned |
| [M6d.10 — uwui designed as a project](stage-6d.md#m6d10--uwui-designed-as-a-project) | Planned |
| [M6d.11 — The move](stage-6d.md#m6d11--the-move) | Planned |
| [M6d.12 — The audit of Stage 6d](stage-6d.md#m6d12--the-audit-of-stage-6d) | Planned |

## M6d.9a1a — The new wire, under today's meaning

Started on 2026-11-12, the first fifth of M6d.9a
([ADR-0246](adr/0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md)), under
[the stage's rules](stage-6d.md#stage-6d--uwui-leaves-the-engine).

- **Goal.** The engine's channel speaks the core's session, each request doing what it does now.
- **Crates.** `uwui-agent`, new; `slime-channel`, `slime-app`, `slime-editor`, `slime-mcp`, with
  the owner's go for `slime-app` and `slime-editor`.
- **Delivers.**
  - `uwui-agent`: the session without input or output, both ends, JSON-RPC a message a line,
    bytes announced in an envelope; a first message naming the agent, its client, the carrier's
    secret and the families with their versions, served or refused one by one.
  - `slime-channel`: its requests in versioned families, through `serde`; its listener and client
    the desktop's carrier, with the token; neither `slime-reflect` nor `slime-serialize`.
  - `slime-mcp` naming its agent as its MCP client names itself; Play naming the editor.
  - Tests of the session over bytes and over the loopback, which Windows runs.
- **Done when.** Every MCP session test passes unchanged; a tool of another family's version is
  served the rest; a program logs the agent that connected; CI is green.
- **Measured.** At rest, with the owner's go: an answer of the channel, the median of 21 on the
  loopback, against 0.14 ms before.

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
- **Layouts migrated** (M6d.3c2): the editor's layout as kept before 0.7, on Linux and in the
  Windows virtual machine, shown as it was at the first start of an editor of 0.7, then kept in
  `<program's name>.layout.kdl`; an editor before 0.7 started after it, which keeps its own.
- **The interface after a lost device, on Direct3D 12** (M6d.4b1a, M6d.4b1b): in the Windows
  virtual machine, *Hunna Wood*'s button that loses the GPU device, with the dev overlay open, the
  loss reported through `slime-gfx`'s door, then the overlay drawn again on the new device.

## Known debt

The known debt still open, 182 items, is in [`debt.md`](debt.md).
