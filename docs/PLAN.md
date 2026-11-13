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
- Done: M6d.3, every document of uwui in KDL, closed on 2026-11-12
  ([ADR-0241](adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)):
  [M6d.3a to M6d.3c3](stage-6d.md#m6d3a--uwuis-reader-of-kdl).
- In parallel, the owner's choice of 2026-11-11: two tracks at most, each milestone in its own
  session.
  - Track A, uwui's core, in the main checkout: M6d.3a to M6d.3c3, done; then M6d.9a, cut in
    five on 2026-11-12
    ([ADR-0246](adr/0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md)):
    done, [M6d.9a1a](stage-6d.md#m6d9a1a--the-new-wire-under-todays-meaning), the new wire;
    then [M6d.9a1b](stage-6d.md#m6d9a1b--requests-in-flight-and-events), cut in three on
    2026-11-13 ([ADR-0250](adr/0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md)):
    done, [M6d.9a1b1](stage-6d.md#m6d9a1b1--requests-in-flight), requests in flight; in
    progress, [M6d.9a1b2](#m6d9a1b2--events), events.
  - Track B, in a worktree: [M6d.6](stage-6d.md#m6d6--a-series-of-headless-compositors),
    done; then [M6d.4](stage-6d.md#m6d4--the-canvas-in-uwui-on-wgpu), the canvas in uwui on
    wgpu, closed on 2026-11-12: [M6d.4a](stage-6d.md#m6d4a--the-canvass-prototype-and-its-adr),
    [M6d.4b1a](stage-6d.md#m6d4b1a--the-canvas-moved-into-uwui-render),
    [M6d.4b1b](stage-6d.md#m6d4b1b--the-canvas-on-wgpu) and
    [M6d.4b2](stage-6d.md#m6d4b2--the-canvass-layout-chosen), its layout chosen; then M6d.5,
    the host's API, cut in five on 2026-11-12
    ([ADR-0248](adr/0248-the-hosts-api-cut-in-five-built-where-it-is-read.md)): done,
    [M6d.5a](stage-6d.md#m6d5a--the-apis-adr-and-its-table), the API decided item by item
    ([ADR-0249](adr/0249-the-hosts-api-its-items-and-their-table.md)), and
    [M6d.5b](stage-6d.md#m6d5b--the-times), the times
    ([ADR-0251](adr/0251-the-engines-host-clock-and-its-times.md)); then
    [M6d.5c](stage-6d.md#m6d5c--input-and-output-in-uwui), cut in three on 2026-11-13
    ([ADR-0253](adr/0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md)): in progress,
    [M6d.5c1](#m6d5c1--events-and-outputs-on-one-target).
  - M6d.5's parts run beside track A in ADR-0248's order; ADR-0249 fixed the three seams on
    2026-11-13: one waker, meaning followed by each reader, an element's capture; M6d.9a1b3
    follows M6d.5c.
  - M6d.10 to M6d.12 run alone.
  - Timed measures run one track at a time; shared files are edited by one session at a time.
- Waiting for the owner: the deletion of the branches `m6d1b-bench`, `m6d4a-prototype` and
  `m6d5a-prototype`, the last two kept while measurements link to their code; the mirror of the game's assets, a private repository, which `scripts/fetch-assets.sh`
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
| [M6d.5a — The API's ADR and its table](stage-6d.md#m6d5a--the-apis-adr-and-its-table) | Done |
| [M6d.5b — The times](stage-6d.md#m6d5b--the-times) | Done |
| [M6d.5c1 — Events and outputs on one target](#m6d5c1--events-and-outputs-on-one-target) | In progress |
| [M6d.5c2 to M6d.5e — The rest of M6d.5](stage-6d.md#m6d5c2--the-clipboard-and-the-hosts-questions) | Planned |
| [M6d.6 — A series of headless compositors](stage-6d.md#m6d6--a-series-of-headless-compositors) | Done |
| [M6d.7 — The desktop host: its window and its frames](stage-6d.md#m6d7--the-desktop-host-its-window-and-its-frames) | Planned |
| [M6d.8 — The desktop host: clipboard, input methods, accessibility](stage-6d.md#m6d8--the-desktop-host-clipboard-input-methods-accessibility) | Planned |
| [M6d.9a1a — The new wire, under today's meaning](stage-6d.md#m6d9a1a--the-new-wire-under-todays-meaning) | Done |
| [M6d.9a1b1 — Requests in flight](stage-6d.md#m6d9a1b1--requests-in-flight) | Done |
| [M6d.9a1b2 — Events](#m6d9a1b2--events) | In progress |
| [M6d.9a1b3 to M6d.9a3b — The rest of M6d.9a](stage-6d.md#m6d9a1b3--long-lists-and-the-interfaces-family) | Planned |
| [M6d.9b — Actions declared once](stage-6d.md#m6d9b--actions-declared-once) | Planned |
| [M6d.10 — uwui designed as a project](stage-6d.md#m6d10--uwui-designed-as-a-project) | Planned |
| [M6d.11 — The move](stage-6d.md#m6d11--the-move) | Planned |
| [M6d.12 — The audit of Stage 6d](stage-6d.md#m6d12--the-audit-of-stage-6d) | Planned |

## M6d.5c1 — Events and outputs on one target

Started on 2026-11-13, the first third of M6d.5c ([ADR-0253](adr/0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md)), under
[the stage's rules](stage-6d.md#stage-6d--uwui-leaves-the-engine).

- **Goal.** A host gives uwui its input as events and reads what uwui asks of it as outputs.
- **Crates.** `uwui`, `slime-app` (`interface.rs`; small steps in `app.rs`, `game.rs`,
  `windowed.rs`), `slime-editor` and `slime-devui` (their tests), *Hunna Wood*'s overlay, the
  examples' tests, the benchmarks.
- **Delivers.**
  - `uwui::host`: `Ui::event`, each event with its target, its time and its source, answered at
    once; the pointer, the wheel, the modifiers, the keys, input methods' composition and commit;
    `Ui::set_event_time` and the seven methods of input gone.
  - `Ui::outputs` for each target, only what changed: the cursor, the input method on as a
    field's input starts, its caret's box; `Ui::cursor` and `Ui::text_input` gone.
  - `host::Person`, a person's input in tests, behind uwui's feature `test-input`.
  - `slime-app` giving the moves of a frame as one, and showing the outputs after each frame.
  - A move walking the tree once, for the hover and the rest.
- **Done when.** The engine's input reaches uwui through the events; uwui's, the editor's and the
  dev overlay's tests of input pass; CI is green.
- **Measured.** A pointer's move over the editor's hierarchy, timed and counted.

## M6d.9a1b2 — Events

Started on 2026-11-13 ([ADR-0254](adr/0254-events-declared-in-the-welcome-replaced-in-the-carrier-published-at-the-frames-end.md)).

- **Goal.** A program sends a connection the changes it subscribed to.
- **Crates.** `uwui-agent`, `slime-channel`, `slime-app`, `slime-editor`.
- **Delivers.** `subscribe` and `unsubscribe`, the events listed in the welcome; a cell for each
  subscription; `play/time`, published at the frame's end; Play on one queue, without polling.
- **Done when.** Play reads the played game's time by events; a tool that stops reading keeps the
  game's memory flat; CI is green.
- **Measured.** An event to the tool, median and slowest of 21, alone and behind a capture; the
  channel's answer; no wake at rest with a subscription.

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
- **A suspend counted** (M6d.5b): the editor on Linux and in the Windows virtual machine, a
  notification shown, the machine suspended for a minute, the pointer moved as it wakes: the
  notification gone at that frame, and the log telling the time suspended.

## Known debt

The known debt still open, 185 items, is in [`debt.md`](debt.md).
