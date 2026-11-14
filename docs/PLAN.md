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

- Stage 6d, uwui leaving the engine, for 0.7: [`stage-6d.md`](stage-6d.md), its whole plan.
- Done: M6d.0a to M6d.3c3, closing M6d.1, how an application is written; M6d.2, typed styles
  ([ADR-0238](adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md)); and M6d.3, every
  document of uwui in KDL ([ADR-0241](adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)).
- Two tracks at most, the owner's choice of 2026-11-11, each milestone in its own session:
  - Track A, uwui's core, in the main checkout: M6d.9a, cut in five
    ([ADR-0246](adr/0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md)):
    done, M6d.9a1a to [M6d.9a1b3a](stage-6d.md#m6d9a1b3a--the-interfaces-family-in-uwui); in
    progress, [M6d.9a1b3b1](#m6d9a1b3b1--lists).
  - Track B, in a worktree: M6d.6, M6d.4 and M6d.5a to M6d.5c3b, done, M6d.5c whole, and
    M6d.5d1; next, [M6d.5d2](stage-6d.md#m6d5d2--the-translation-in-uwui-accesskit).
  - M6d.5's parts run beside track A in ADR-0248's order, the seams fixed by ADR-0249.
  - M6d.10 to M6d.12 run alone.
  - Timed measures run one track at a time; shared files are edited by one session at a time.
- Waiting for the owner: deleting the branches `m6d1b-bench`, `m6d4a-prototype` and
  `m6d5a-prototype`, the last two linked by measurements; the
  [checks by hand](#checks-by-hand-waiting-for-the-owner).
- After Stage 6d: uwui's stages, Stage 6e, the rest of Stage 6c ([`stage-6c.md`](stage-6c.md)),
  then Stage 7 ([`ROADMAP.md`](ROADMAP.md#stage-7--runtime-systems--08)).

## Status

| Milestone | Status |
|---|---|
| [M6d.0a to M6d.3c3, and C1](stage-6d.md#where-it-stands) | Done |
| [M6d.4a to M6d.4b2 — The canvas in uwui, on wgpu](stage-6d.md#m6d4--the-canvas-in-uwui-on-wgpu) | Done |
| [M6d.5a to M6d.5d1 — The host's API, its first parts](stage-6d.md#m6d5--the-hosts-api) | Done |
| [M6d.5d2 — The translation in uwui-accesskit](stage-6d.md#m6d5d2--the-translation-in-uwui-accesskit) | Planned |
| [M6d.5e — slime-app a host through the API alone](stage-6d.md#m6d5e--slime-app-a-host-through-the-api-alone) | Planned |
| [M6d.6 — A series of headless compositors](stage-6d.md#m6d6--a-series-of-headless-compositors) | Done |
| [M6d.7 — The desktop host: its window and its frames](stage-6d.md#m6d7--the-desktop-host-its-window-and-its-frames) | Planned |
| [M6d.8 — The desktop host: clipboard, input methods, accessibility](stage-6d.md#m6d8--the-desktop-host-clipboard-input-methods-accessibility) | Planned |
| [M6d.9a1a to M6d.9a1b2 — The new wire, requests in flight, events](stage-6d.md#m6d9a1a--the-new-wire-under-todays-meaning) | Done |
| [M6d.9a1b3a — The interface's family in uwui](stage-6d.md#m6d9a1b3a--the-interfaces-family-in-uwui) | Done |
| [M6d.9a1b3b1 — Lists](#m6d9a1b3b1--lists) | In progress |
| [M6d.9a1b3b2 to M6d.9a3b — The rest of M6d.9a](stage-6d.md#m6d9a1b3b2--the-editors-journal) | Planned |
| [M6d.9b — Actions declared once](stage-6d.md#m6d9b--actions-declared-once) | Planned |
| [M6d.10 — uwui designed as a project](stage-6d.md#m6d10--uwui-designed-as-a-project) | Planned |
| [M6d.11 — The move](stage-6d.md#m6d11--the-move) | Planned |
| [M6d.12 — The audit of Stage 6d](stage-6d.md#m6d12--the-audit-of-stage-6d) | Planned |

## M6d.9a1b3b1 — Lists

Started on 2026-11-14.

- **Goal.** Entities, elements and the log read by pages.
- **Crates.** `uwui-agent`, `uwui`, `slime-world`, `slime-channel`, `slime-app`, `slime-mcp`.
- **Delivers.** [ADR-0260](adr/0260-a-long-list-read-by-pages-a-cursor-bound-to-its-maker-pages-bounded-by-time-m6d9a1b3b-cut-in-two.md)'s cursor, bounds and cut texts; faults' codes both ways and `STALE`;
  `world` 2, `interface` 3; the MCP server's lists paged.
- **Done when.** A list passes an element larger than the tool's line; a world replaced makes its
  cursor stale; seven agents listing keep the frame's start in budget; CI is green.
- **Measured.** At rest, with the owner's go: a page of a thousand elements; seven agents' frames.

## Ideas waiting for their milestone

From two outside reviews, kept by the owner on 2026-11-10; each joins its milestone at its start.

- Stage 7's planning: the very modest laptop measured on
  [ADR-0216](adr/0216-light-geometry-and-the-gpus-floor.md)'s common floor before the renderer
  stands on it; one memory budget for every load and renderer ([`debt.md`](debt.md));
  the simulation's parallel run tried on one case before its systems.
- uwui's first stage: a channel's capacity, and its messages told apart, last value, kept events,
  large changes in parts ([ADR-0211](adr/0211-the-executor-and-other-threads-through-channels.md)).
- A milestone that fixes a huge text's costs in a field, the owner's choice of 2026-11-13
  ([ADR-0255](adr/0255-the-clipboard-through-the-outputs-and-the-hosts-questions.md)): placed
  when Stage 6d's audit plans uwui's first stage.
- Later: cells loaded under a budget before Stage 10a's formats; a view's durable state keyed
  apart from its path; themes' parts tried on hard components; few concepts to start with; early
  trials of mobile and web life cycles.

## Checks by hand waiting for the owner

The owner makes them later, together, as decided on 2026-10-20. Each milestone adds its own once
done; a check made leaves the list, its result written in its milestone, then in
[`history.md`](history.md). The game runs with `--new --save <a file of its own>`, leaving the
owner's save alone. After Stage 6e's first milestone, a check that names *Hunna Wood* runs on
`games/open-world` ([ADR-0219](adr/0219-test-games-in-place-of-hunna-wood.md)).

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
- **Input methods as the host's outputs** (M6d.5c1): on the owner's Linux desktop with an input
  method of Japanese or Chinese, and in the Windows virtual machine with Microsoft's: a
  composition in a field of the editor, its window beside the caret as it moves; the focus moved
  to another field, then back to the first, mid-composition, the composition ended each time and
  the next one starting clean.
- **The clipboard through the outputs** (M6d.5c2): on the owner's Linux desktop and in the Windows
  virtual machine, copy, cut and paste in a field of the editor, by the keys and between it and
  another program; the editor's own copy and paste of entities; on macOS, whether ⌘V asks the
  person before the engine reads, in a field and for the editor's paste.
- **A suspend counted** (M6d.5b): the editor on Linux and in the Windows virtual machine, a
  notification shown, the machine suspended for a minute, the pointer moved as it wakes: the
  notification gone at that frame, and the log telling the time suspended.
- **Play by events** (M6d.9a1b2): `play.zone` on the owner's desktop, its time to control now from
  the first `play/time` whose `held` is false; in the Windows virtual machine, Play's toolbar
  following a pause made from the dev overlay, and Stop, the game's end seen at once.
- **A screen reader's request on its window** (M6d.5d1): Narrator in the Windows virtual machine
  pressing a button and setting a field of the `interface` example.
- **A covered window at rest** (M6d.5c3b): *Hunna Wood* paused, its dev overlay open, the pointer
  leaving its window as another covers it, in the Windows virtual machine and the macOS one: the
  game asking no frame while covered, and drawing again once shown.

## Known debt

The known debt still open, 187 items, is in [`debt.md`](debt.md).
