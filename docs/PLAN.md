# Plan

This file says what is being worked on now: the current stage's status, the milestone in progress
and how it is checked, and the checks by hand that wait for the owner. A *stage* is a step of the roadmap; a *milestone* is a small, verifiable
step of a stage.

- The stage in progress, whole: [`stage-6d.md`](stage-6d.md).
- The known debt still open: [`debt.md`](debt.md).
- The finished stages and their releases: [`history.md`](history.md).
- Direction beyond this stage: [`ROADMAP.md`](ROADMAP.md).
- Decisions: [`adr/`](adr/README.md).

Every milestone ends with the definition of done in [`AGENTS.md`](../AGENTS.md#definition-of-done),
and is checked on Windows ([ADR-0014](adr/0014-amend-roadmap-for-stage-2-and-windows.md)).

## Current

- Stage 6d, uwui leaving the engine, for 0.7: [`stage-6d.md`](stage-6d.md), its whole plan.
- Done: M6d.0a to M6d.3c3: M6d.1, M6d.2 and M6d.3.
- Two tracks at most, the owner's choice of 2026-11-11, each milestone in its own session:
  - Track A, uwui's core, in the main checkout: M6d.9a, cut in five
    ([ADR-0246](adr/0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md)):
    done, M6d.9a1a to [M6d.9a1b3b2](stage-6d.md#m6d9a1b3b2--the-editors-journal), M6d.9a1
    whole; M6d.9a2 cut in three ([ADR-0269](adr/0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md)): done,
    [M6d.9a2a](stage-6d.md#m6d9a2a--the-library); M6d.9a2b cut in two
    ([ADR-0274](adr/0274-the-engines-families-typed-on-the-cores-way-a-family-not-served-refused-as-it-is-sent-m6d9a2b-cut-in-two.md)): done,
    [M6d.9a2b1](stage-6d.md#m6d9a2b1--the-engines-families-on-the-library); next,
    [M6d.9a2b2](stage-6d.md#m6d9a2b2--the-servers-game-off-its-lock).
  - Track B, in a worktree: M6d.6, M6d.4 and M6d.5 done; M6d.7 cut in six: done,
    [M6d.7a1](stage-6d.md#m6d7a1--the-hosts-core-and-its-wayland-window); next,
    [M6d.7a2](stage-6d.md#m6d7a2--wayland-whole-and-the-host-measured).
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
| [M6d.5a to M6d.5e2 — The host's API](stage-6d.md#m6d5--the-hosts-api) | Done |
| [M6d.6 — A series of headless compositors](stage-6d.md#m6d6--a-series-of-headless-compositors) | Done |
| [M6d.7a1 — The host's core and its Wayland window](stage-6d.md#m6d7a1--the-hosts-core-and-its-wayland-window) | Done |
| [M6d.7a2 to M6d.7e — The rest of M6d.7](stage-6d.md#m6d7a2--wayland-whole-and-the-host-measured) | Planned |
| [M6d.8 — The desktop host: clipboard, input methods, accessibility](stage-6d.md#m6d8--the-desktop-host-clipboard-input-methods-accessibility) | Planned |
| [M6d.9a1a to M6d.9a1b3b2 — The new wire, requests, events, the interface's family](stage-6d.md#m6d9a1a--the-new-wire-under-todays-meaning) | Done |
| [M6d.9a2a — The library](stage-6d.md#m6d9a2a--the-library) | Done |
| [M6d.9a2b1 — The engine's families on the library](stage-6d.md#m6d9a2b1--the-engines-families-on-the-library) | Done |
| [M6d.9a2b2 to M6d.9a3b — The rest of M6d.9a](stage-6d.md#m6d9a2b2--the-servers-game-off-its-lock) | Planned |
| [M6d.9b — Actions declared once](stage-6d.md#m6d9b--actions-declared-once) | Planned |
| [M6d.10 — uwui designed as a project](stage-6d.md#m6d10--uwui-designed-as-a-project) | Planned |
| [M6d.11 — The move](stage-6d.md#m6d11--the-move) | Planned |
| [M6d.12 — The audit of Stage 6d](stage-6d.md#m6d12--the-audit-of-stage-6d) | Planned |

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

From the review of the architecture of 2026-11-16, its items in
[`debt.md`](debt.md#left-by-the-review-of-the-architecture-before-m6d10):

- M6d.10: agents' families served over a range of versions; one windowing layer or two
  ([ADR-0270](adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md)).
- Before Stage 7's systems: an ADR on the simulation's contract, its parallel access, its seeded
  randomness and its order of iteration; a typed copy of the world; a host without window or GPU.
- For the owner: the contributors' files and terms, the names reserved, and shorter documents:
  ADRs superseded rather than amended, a short overview, the changelog in fragments.

## Checks by hand waiting for the owner

The owner makes them later, together (2026-10-20). A milestone adds its own once done; a check
made leaves the list, its result in its milestone, then in [`history.md`](history.md). The game
runs with `--new --save <a file of its own>`. After Stage 6e's first milestone, a check naming
*Hunna Wood* runs on `games/open-world` ([ADR-0219](adr/0219-test-games-in-place-of-hunna-wood.md)).

- **The very modest laptop** (M6d.0c): its inventory, given on 2026-11-06 from `lshw -short` and
  `inxi -CGmSz`, lacks its display's size and rate, its storage and its Vulkan driver, which
  `nix-shell -p inxi pciutils vulkan-tools --run "inxi -GDz; vulkaninfo --summary"` and
  `niri msg outputs` give; then its measures.
- **The music player's Android prototype** (M6d.0c): written in Compose and measured on the Pixel.
- **Checkpoint A, in the Windows virtual machine** (M6c.1a): the editor's window; flying through
  the zone, selecting, rearranging it and playing it; the raw motion, the cursor held, the game's
  child process and the system's firewall, the dialogs.
- **The system's dialog, the cursor held and the raw motion** (M6c.1b): on the owner's Linux
  desktop and in the Windows virtual machine; the cursor and the motion also on X11 through
  XWayland.
- **Flying and orbiting in the zone** (M6c.2a): on the owner's Linux desktop and through XWayland.
- **Sixteen passes to a submission** (M6c.2a): on Direct3D 12, in the Windows virtual machine.
- **Picking in the zone** (M6c.2b): on the owner's Linux desktop and X11 through XWayland; Alt and a click
  with a mouse of a fine resolution, which must not read it as an orbit; the outline on Direct3D
  12, in the Windows virtual machine.
- **Two viewports and Blender's way** (M6c.2c): on Linux and in the Windows virtual machine.
- **The handles** (M6c.3a): their feel, their hit areas and the snapping, in the yard and the zone,
  and in the Windows virtual machine; Escape during a drag, which tests reach through the model,
  not through a key of the window.
- **Creating and arranging** (M6c.3b): creating, duplicating, copying between the yard and the
  zone, grouping and nudging, on Linux and in the Windows virtual machine.
- **Placing prefabs and the history panel** (M6c.3c): in the yard and the zone, and in the Windows
  virtual machine.
- **Play, and edits while playing** (M6c.4a, M6c.4b): Play, Pause, Step and Stop with *Hunna
  Wood* and the yard, on Linux and in the Windows virtual machine.
- **Building the game again** (M6c.4c): a change of *Hunna Wood*'s rules built and started again,
  on Linux and in the Windows virtual machine; there, a build while the game runs, which Windows
  may refuse, and no console window opening.
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
  by case only, cooked twice: whether its ID changes, and whether `slime-cook watch`
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
  virtual machine, *Hunna Wood*'s button that loses the device, its dev overlay open: the loss
  reported through `slime-gfx`'s door, the overlay drawn again on the new device.
- **Input methods as the host's outputs** (M6d.5c1): on the owner's Linux desktop with an input
  method of Japanese or Chinese, and in the Windows virtual machine with Microsoft's: a
  composition in a field of the editor, its window beside the moving caret; the focus moved to
  another field and back, mid-composition, each composition ended and the next starting clean.
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
  leaving its window as another covers it, in the Windows virtual machine, the macOS one and on
  bare X11 (M6d.5e1): no frame asked while covered, drawn again once shown.
- **Each window's size and scale** (M6d.5e2): the editor in the Windows virtual machine moved
  between screens of two scales, and its scale changed while it runs: laid out again at once,
  sharp; minimized and restored as it was.
- **An application on uwui's desktop host** (M6d.7a1): `cargo run -p uwui-desktop --example
  application` on the owner's desktop: its window, its bar or none, the pointer on its buttons,
  list and dialog, then at rest; closed by its window's button, then by Ctrl+C in its terminal.
- **Focus, a lost surface** (M6d.5e1): the editor on Linux and in the Windows virtual machine,
  left by Alt and Tab, Alt held or mid-drag: no bar in focus, nothing dropped; *Hunna Wood*'s
  button that loses the surface, on Direct3D 12: drawn again.

## Known debt

The known debt still open, 216 items, is in [`debt.md`](debt.md).
