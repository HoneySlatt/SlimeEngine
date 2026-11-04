# Plan

This file says what is being worked on now: the current stage, its milestones, their status, how
each one is checked, and the known debt. A *stage* is a step of the roadmap; a *milestone* is a
small, verifiable step of a stage.

- The finished stages and their releases: [`history.md`](history.md).
- Direction beyond this stage: [`ROADMAP.md`](ROADMAP.md).
- Decisions: [`adr/`](adr/README.md).

Every milestone ends with the definition of done in [`AGENTS.md`](../AGENTS.md#definition-of-done):
build, tests, clippy, rustfmt and rustdoc pass, the program was actually run, and the affected
documentation was updated and reread. Every milestone is also checked on Windows
([ADR-0014](adr/0014-amend-roadmap-for-stage-2-and-windows.md)).

## Current

- Stage in progress: Stage 6c, the editor, to be released as 0.7
  ([ADR-0155](adr/0155-the-editors-design.md)).
- Done: M6c.0, M6c.1a, M6c.1b, M6c.2a, M6c.2b, M6c.2c, M6c.3a, M6c.3b, M6c.3c, M6c.4a, M6c.4b,
  M6c.4c, M6c.4d, M6c.5a, M6c.5b, M6c.5c and M6c.5d.
- Checkpoint A: done on Linux; its part in the Windows virtual machine waits for the owner.
- Next: [M6c.5e](#m6c5e--layouts-kept-floating-panels-and-closed-panels), layouts kept, floating
  panels and closed panels.
- Then: M6c.5f, M6c.6, M6c.7, Checkpoint B, M6c.8 and M6c.9.
- Cuts made at a milestone's start, a part cut again at its own start where needed: M6c.1 into 1a
  and 1b; M6c.2 into 2a to 2c; M6c.3 into 3a to 3c; M6c.4 into 4a to 4d; M6c.5 into 5a to 5f.
- M6c.6 is expected to be cut in two, M6c.6a and M6c.6b.
- After Stage 6c: Stage 6d, the interface for applications, to be released as 0.8, decided on
  2026-11-04 ([ADR-0170](adr/0170-applications-on-the-ui-framework.md),
  [ADR-0171](adr/0171-effects-of-the-interface.md),
  [ADR-0173](adr/0173-the-platforms-of-applications.md)).
- Stage 6d's milestones are planned at its start, with the owner.
- After Stage 6d: Stage 7, runtime systems, to be released as 0.9
  ([`ROADMAP.md`](ROADMAP.md#stage-7--runtime-systems--09)).

## Status

| Milestone | Goal | Status |
|---|---|---|
| [M6c.0](#m6c0--before-the-stage) | The development channel answers in milliseconds, CI's checks stay within their limit, and a game that a tool started never outlives it | Done |
| [M6c.1a](#m6c1a--the-window-and-its-operations) | The editor opens a level in a window of its own, where a person and an agent run the same named operations | Done |
| [M6c.1b](#m6c1b--dialogs-the-cursor-held-and-the-gpus-identity) | A person opens a level through the system's dialog, a viewport can hold the cursor and read the mouse's raw motion, and a GPU object cannot act on another `Gpu`'s | Done |
| [M6c.2a](#m6c2a--the-viewport-and-its-navigation) | The level shows in a viewport that the user flies through, drawn again only when what it shows changed | Done |
| [M6c.2b](#m6c2b--picking-and-the-selection) | The user picks in the viewport and sees what is selected outlined | Done |
| [M6c.2c](#m6c2c--a-second-viewport-and-blenders-way) | A second viewport shows the level, and Blender's way is a preset of the keys and the mouse's buttons | Done |
| [M6c.3a](#m6c3a--move-with-handles) | The user moves, turns and scales what is selected with handles, each gesture a single step of the history | Done |
| [M6c.3b](#m6c3b--create-and-arrange) | The user creates, duplicates, deletes, copies, groups and nudges what is selected, each a step | Done |
| [M6c.3c](#m6c3c--place-prefabs-and-see-the-history) | The user places prefabs and sees the history | Done |
| [M6c.4a](#m6c4a--play-the-level) | Play runs the level being edited in the game, steps not written included, paused, stepped and stopped from a toolbar, its log in a console | Done |
| [M6c.4b](#m6c4b--edit-while-playing) | The editor's edits reach the running game, and agents drive both programs | Done |
| [M6c.4c](#m6c4c--build-the-game-again) | Play builds the game before it starts it, and a game whose code changed while it runs is built again and started on a save of its run | Done |
| [M6c.4d](#m6c4d--measure-play) | Play's time to control is measured on the zone and on a level of 100 000 entities | Done |
| [Checkpoint A](#checkpoint-a--the-owner-tries-the-editor) | The owner tries the editor on Linux and in the Windows virtual machine, and the plan is adjusted | Done on Linux; the Windows virtual machine to come |
| [M6c.5a](#m6c5a--the-hierarchy) | The user finds any entity in a hierarchy, which follows the world's tree, folds, selects, renames and searches | Done |
| [M6c.5b](#m6c5b--rearrange-the-hierarchy) | The user reparents entities by dragging them in the hierarchy and with Alt and the arrows | Done |
| [M6c.5c](#m6c5c--the-eye-and-the-lock) | The user hides entities from the viewport and locks them against picking, as the user's state, never the level's | Done |
| [M6c.5d](#m6c5d--regions-of-tabs) | The user arranges the panels in regions of tabs | Done |
| [M6c.5e](#m6c5e--layouts-kept-floating-panels-and-closed-panels) | The user's layout outlives the editor, and panels float and close | Planned |
| [M6c.5f](#m6c5f--the-inspector) | The user changes the components of what is selected in an inspector, and keeps a value of the running game | Planned |
| [M6c.6](#m6c6--levels-of-several-files-and-links) | A level is made of several files, which the editor writes together, and an entity refers to entities of other files | Planned |
| [M6c.7](#m6c7--build-from-a-kit) | The user builds a level from a kit of prefabs found in an asset browser, and *Hunna Wood*'s zone is made so | Planned |
| [Checkpoint B](#checkpoint-b--the-exit-rehearsal) | The owner builds a part of the zone again by hand in the editor, plays it, writes it and opens it again: the exit criterion | Planned |
| [M6c.8](#m6c8--panels-dragged-to-dock) | The user drags panels and tabs to arrange the editor; may move to Stage 11 | Planned |
| [M6c.9](#m6c9--windows-check-documentation-release-07) | Stage 6c is done, checked on Windows, and released as 0.7 | Planned |

## Stage 6c — The editor

Exit criterion ([`ROADMAP.md`](ROADMAP.md#stage-6c--the-editor--07)): a level of the game is
built, saved and reloaded entirely in the editor, with undo and redo on every operation; the editor
uses no private engine API. Released as 0.7 ([ADR-0131](adr/0131-stage-6b-and-stage-6c.md)).

[ADR-0155](adr/0155-the-editors-design.md) records the owner's decisions of 2026-10-27: the
editor's design, the milestones, their order, the two checkpoints, one release and the
dependencies. Each milestone delivers what ADR-0155 decides for it, and:

- records the design of a part in a new ADR, which amends ADR-0155 where it changes it;
- adds a dependency only when it first uses it, and checks what it adds to CI's build;
- ends with an agent's session in `scripts/check.sh`, in a headless Weston when windowed, and image
  tests of the panels it adds;
- records how long CI's jobs take, each within its limit of 20 minutes;
- may be cut in two at its start.

Out of scope: panels in windows of their own (one window per process); a highlight that follows
the pointer; the world updated in place keeping a game's handles (Stage 8); files dropped on
Wayland, and placed where they fall, which wait for winit 0.31 (a milestone of its own, not holding
0.7 back); named layouts; a
start page or a manager of projects; modal keys G, R and S; a wireframe of the whole scene; a fix
of [ADR-0041](adr/0041-render-graph.md)'s stall, unless the owner brings it in; editors of
materials, animation, effects and sound (Stage 11); a game's own editing tools (Stage 8); story
layers (Stage 9b); cells and streaming (Stage 10a).

Open risks (ADR-0155's context gives their figures):

- ADR-0041's stall makes a view drawn again cost milliseconds of CPU without resizable BAR, on the
  owner's machine too; the editor and the game at once, and Windows, are not measured.
- On Windows, a running executable may block Play's link of the game.
- Not measured: the outline and the handles at 3840 × 2160 with many entities selected; thumbnails
  of thousands of meshes; docking's drags.
- The stage's size: ADR-0155's study counted 31 000 to 39 000 lines of Rust with their tests.
- winit 0.31, in beta for months, and `accesskit_winit`'s release for it.

In the milestones done, "Debt:" names entries of [Known debt](#known-debt) by their titles.

### M6c.0 — Before the stage

Done 2026-10-27, CI green on `bb4552b`: the development channel answers in 0.14 ms, not 82 ms; a
game that a tool started ends when its standard input closes; `scripts/check.sh` runs in three
parts, which CI runs side by side.

### M6c.1a — The window and its operations

Done 2026-10-28, [ADR-0156](adr/0156-the-editors-window-and-its-operations.md), CI green on
`a20c15c`: the editor's window, drawn on demand, whose menus, shortcuts, palette and agents run one
registry of operations.

### M6c.1b — Dialogs, the cursor held and the GPU's identity

Done 2026-10-28, [ADR-0157](adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md), CI green
on `49620a2`: the system's dialog opens a level, the cursor is held and the raw motion read, and a
GPU object used with another `Gpu` panics. Debt: "Input not reported yet".

### M6c.2a — The viewport and its navigation

Done 2026-10-28, [ADR-0158](adr/0158-the-viewport.md), CI green on `7d51d93`: the level in a
viewport, flown, orbited and panned, drawn only when what it shows changed. Debt: "The editor's
viewport shows every light", "Sixteen passes per submission".

### M6c.2b — Picking and the selection

Done 2026-10-28, [ADR-0159](adr/0159-picking-and-the-selection.md), CI green on `00766ea`: a
click, a marquee, and Alt and a click pick; the selection outlined; two modes of debugging. Debt:
"Picking in the editor's viewport".

### M6c.2c — A second viewport and Blender's way

Done 2026-10-28, [ADR-0160](adr/0160-two-viewports-and-blenders-way.md), CI green on `271cdd7`: a
second viewport, and Blender's keys and buttons as a preset. Debt: "Two viewports build their
frames apart".

### M6c.3a — Move with handles

Done 2026-10-28, [ADR-0161](adr/0161-the-open-step-and-the-handles.md), CI green on `55258a9`:
handles move, turn and scale, with snapping, each gesture one step; End drops on what lies below.
Debt: "The editor's handles".

### M6c.3b — Create and arrange

Done 2026-10-28, [ADR-0162](adr/0162-creating-copying-and-arranging.md), CI green on `95b7bbc`:
create, duplicate, delete, copy and paste, group, nudge and repeat, each a step. Debt: "Creating
and arranging in the editor".

### M6c.3c — Place prefabs and see the history

Done 2026-10-28, [ADR-0163](adr/0163-placing-prefabs-and-the-history-panel.md), CI green on
`b5b350b`: prefabs placed from the palette, and a history panel. Debt: "Placing prefabs and the
history panel".

### M6c.4a — Play the level

Done 2026-10-28, [ADR-0164](adr/0164-play.md), CI green on `dd44fe6`: Play runs the level in the
game, its unwritten steps included, from a toolbar, its log in a console. Debt: "Play".

### M6c.4b — Edit while playing

Done 2026-10-28, [ADR-0165](adr/0165-editing-while-playing.md), CI green on `ff64197`: steps reach
the running game, and agents drive both programs. Debt: "Editing while playing".

### M6c.4c — Build the game again

Done 2026-10-28, [ADR-0166](adr/0166-building-the-game-again.md), CI green on `b271491`: Play
builds the game, and builds it again as its code changes, on a save of its run. Debt: "Building the
game again".

### M6c.4d — Measure Play

Done 2026-10-28, CI green on `cb8d694`: two benchmarks measure Play's time to control
([`docs/measurements.md`](measurements.md)). Debt: "Play's time to control".

### Checkpoint A — The owner tries the editor

- **What.** The owner, for about an hour, on Linux and in the Windows virtual machine: flies
  through the zone, selects, rearranges it and plays it; the raw motion, the cursor held, the
  game's child process and the system's firewall, the dialogs.
- **Result.** A list of frictions, and this plan adjusted.
- **Linux, 2026-10-29.** All worked. Flying was slow: it now starts at a speed that suits the
  level, kept per level and shown in the status bar. No cut; "keep this value" moved to M6c.5f.
- **Windows virtual machine.** Waits for the owner.

### M6c.5a — The hierarchy

Done 2026-10-29, [ADR-0167](adr/0167-the-hierarchy.md), CI green on `2f78cce`: a hierarchy that
folds, selects, renames and searches, within a list's budget at 100 000 entities. Debt: "The
hierarchy".

### M6c.5b — Rearrange the hierarchy

Done 2026-10-29, [ADR-0168](adr/0168-rearranging-the-hierarchy.md), CI green on `b7952a9`:
reparenting by drags and by Alt and the arrows; drag and drop in `slime-ui`; G32 settled. Debt:
"Rearranging the hierarchy".

### M6c.5c — The eye and the lock

Done 2026-11-04, [ADR-0169](adr/0169-the-eye-and-the-lock.md), CI green on `b2e1c1a`: entities
hidden from the viewport and locked against picking, as the user's state. Debt: "The eye and the
lock".

### M6c.5d — Regions of tabs

Done 2026-11-04, [ADR-0172](adr/0172-regions-of-tabs.md), CI green on `06faca1`: `slime-ui`'s dock
of splits and tabs, and the editor's panels in regions, moved by "Move to". Debt: "Regions of
tabs".

### M6c.5e — Layouts kept, floating panels and closed panels

- **Goal.** The user's layout outlives the editor, and panels float and close.
- **Crates.** `slime-ui`, `slime-editor`.
- **Delivers.** The layout saved per user and per project in the user's state directory, a game's
  editor package giving the project's default; read leniently, set aside when it cannot be read,
  the user told; "Reset layout"; the default's version, a newer default replacing a layout saved
  against an older one; floating panels, which "Move to" reaches; a closed panel keeping its place,
  to come back there ([ADR-0172](adr/0172-regions-of-tabs.md)).
- **Done when.** A layout that cannot be read gives the default; a layout changed comes back at the
  next start; an agent floats a panel, closes one and opens it again where it was; CI is green.
- **How to test.** `cargo test -p slime-ui -p slime-editor`; the MCP server's session tests.
- **By hand.** A layout kept across starts, a panel floated, closed and opened again, on Linux and
  in the Windows virtual machine.

### M6c.5f — The inspector

- **Goal.** The user changes the components of what is selected in an inspector, and keeps a value
  of the running game.
- **Crates.** `slime-devui`, `slime-editor`.
- **Delivers.** `slime-devui`'s builder of an entity's components made public, and the inspector
  on it: components added and removed; several entities selected, a mixed value shown as "—", a
  check box's third state; overrides marked by a bar and a bold label, Revert and Apply for each
  field; driven fields read-only; the interim world panel retired. "Keep this value", placed here by
  the owner on 2026-10-29: a value read in the game that Play runs made a step of the level; its
  design decides what a step keeps, a field's value, a component's or an entity's.
- **Done when.** An agent reverts an override and keeps a value of the running game; image tests of
  the inspector; CI is green.
- **How to test.** `cargo test -p slime-devui -p slime-editor`.
- **By hand.** The inspector, read by Orca on Linux and by Narrator in the Windows virtual machine.
- **Debt settled.** A check box without a third state; an entity that a run created, wrote into its
  level and destroyed coming back
  ([ADR-0124](adr/0124-driven-components-keys-and-created-entities-in-levels.md)), which a value
  that the editor keeps settles.

### M6c.6 — Levels of several files, and links

- **Goal.** A level is made of several files, which the editor writes together, and an entity
  refers to entities of other files.
- **Crates.** `slime-world`, `slime-serialize`, `slime-reflect`, `slime-cook`, `slime-devui`,
  `slime-editor`; `.gitattributes`; the examples' yard and *Hunna Wood*'s zone.
- **Delivers.** As [ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md) decides too, in
  two parts, each likely a milestone; the first part's ADR amends
  [ADR-0091](adr/0091-scene-documents-and-placements.md) for the layout.
  - *M6c.6a, levels of several files.* It starts with two unproven risks: writing several files
    whole or not at all, beside `World::carry` and saves per scene, and opening leniently by
    splicing text back. The new layout of scene files, whole, in one change: the level's parts in a
    table of their own, former identities, the absolute identities that `EntityRef` writes, and
    whether a file says the parents and the order of a placement's entities. The core over several
    files: a step across files, the journal's entries naming their files, every file written or
    those named, all or none. A level holding a component or a field that the program does not
    know opens, its text written back byte for byte. A check before writing that the placements of
    a changed prefab still spawn. Prefabs changed on disk followed; a prefab opened in context. The
    yard and the zone written in the new layout.
  - *M6c.6b, links.* `EntityRef`, and its picker in the inspector: an eyedropper in the viewport, a
    drag from the hierarchy, and a search over the parts loaded or not; a prefab made of a
    selection; Apply to prefab.
- **Done when.** `slime-world`'s round trips on fixtures of the new layout; tests of the core's
  steps across files; a failure injected between two renames leaves the files as they were, or
  names those written; one edit changes one hunk of a file; a level with an unknown component
  opened, changed and written back with that component's text unchanged; an agent splits the yard
  into parts and links an entity across two; Play's time to control measured again in a debug
  build on a level of 100 000 entities in parts, to say whether the editor's writing or the game's
  spawning needs work, as the owner chose on 2026-10-29; CI is green.
- **How to test.** `cargo test -p slime-world -p slime-serialize -p slime-editor`; the MCP server's
  session tests.
- **By hand.** The owner moves an entity from one part of the zone to another, and links two parts.
- **Debt settled.** G31, G40, G42, G43 and G45; the parents and the order of a placement's
  entities; the editor not following the prefabs that change on disk.

### M6c.7 — Build from a kit

- **Goal.** The user builds a level from a kit of prefabs found in an asset browser, cooked by the
  editor as its sources change, and *Hunna Wood*'s zone is made so.
- **Crates.** `slime-editor`, `slime-cook`, `slime-assets`, `slime-render`, `slime-ui-render`,
  `slime-platform`; `games/hunnawood`, its zone's Blender script, its tests and its save fixture.
- **Delivers.**
  - The asset browser: an index of the cooker's manifest and a scan of the sources, by folder and
    kind, with a search.
  - Thumbnails: a texture's small level of its KTX2, a mesh or a prefab drawn offscreen, a few a
    frame and only for the tiles that show, encoded as PNG with `Compression::Fast`, cached in the
    user's cache directory under the cooked file's name.
  - Assets dragged into a viewport; stamp mode, which may be cut; New level.
  - Cooking in the editor's process, at the background priority, with its progress, once two cooks
    in one process no longer share their temporary files (G49).
  - The tools' watch told of changes by notify 8.2.1, once polling's cost is measured on Windows;
    a scan again when its queue overflows; polling as its fallback.
  - Files dropped on Windows and X11 imported; the interface's images asked of the asset cache.
  - *Hunna Wood*'s kit: its Blender script exports pieces of terrain and one prefab per variant of
    tree, rock and prop, with its collider; an agent converts the scatter into placements once,
    through the editor's operations; the zone's tests on the kit, and a new save fixture, which
    [ADR-0100](adr/0100-updates-of-shipped-games.md) allows before the game's first release, said
    in the game's README.
  - In the known debt: winit's drag and drop makes Windows' main thread a single-threaded COM
    apartment, which Stage 7's audio must not start as a multi-threaded one.
- **Done when.** Thumbnails found in the cache after a restart; tests of the watch: a file written,
  renamed and removed, and a queue that overflowed; an agent places an asset by its ID and cooks;
  the zone's tests pass on the kit, with the new fixture; CI is green.
- **How to test.** `cargo test -p slime-editor -p slime-cook -p slime-assets`;
  `scripts/fetch-assets.sh`;
  `blender --background --factory-startup --python games/hunnawood/zone/build_zone.py`;
  `cargo test -p hunnawood -p hunnawood-editor -- --ignored`.
- **By hand.** The browser, its thumbnails and stamping, on Linux; G20, in
  [the list](#checks-by-hand-waiting-for-the-owner).
- **Debt settled.** G16, G20 with the owner's check, G23, G49 to G53 and G56; the interface's images
  given by the program.

### Checkpoint B — The exit rehearsal

- **What.** The owner, on Linux and in the Windows virtual machine: erases the zone's campfire
  clearing, builds it again by hand in the editor, plays it, writes it, opens it again, and undoes
  and redoes through it all.
- **Result.** The exit criterion met, or what keeps it from being met.

### M6c.8 — Panels dragged to dock

- **Goal.** The user drags panels and tabs to arrange the editor. If the stage runs long, the
  milestone moves to Stage 11, with an amendment of ADR-0131 and the roadmap, the regions of M6c.5d
  serving meanwhile.
- **Crates.** `slime-ui`, `slime-editor`; `games/hunnawood-editor`.
- **Delivers.** Zones to drop a panel or a tab on, shown as it is dragged; tabs moved and closed;
  floating panels docked again; closed panels coming back to their places; a least size for
  panels; a project's default layout, which a game's editor package gives.
- **Done when.** `slime-ui`'s tests of the layout's rules after random edits; a layout saved against
  an older default upgraded; a layout that cannot be read gives the default and a notification; an
  agent moves a panel by its operation; CI is green.
- **How to test.** `cargo test -p slime-ui -p slime-editor`.
- **By hand.** Panels docked by drag on the owner's Linux desktop and in the Windows virtual machine.
- **Debt settled.** Tabs neither moved nor closed; two panels without a least size; a floating
  panel that does not dock.

### M6c.9 — Windows check, documentation, release 0.7

- **Goal.** Stage 6c is done, checked on Windows, and released as 0.7.
- **Delivers.** The owner's check, in the Windows virtual machine, of the programs that CI builds,
  with the checks that wait in [the list](#checks-by-hand-waiting-for-the-owner) if the owner makes
  them then; the documents reread as a newcomer would read them; `CHANGELOG.md`'s section of 0.7;
  the release, as [Releasing](#releasing) says; Stage 6c moved to [`history.md`](history.md);
  Stage 6d planned with the owner.
- **Done when.** The owner ran *Hunna Wood*'s editor on Windows; the documents are reread and
  corrected; CI is green on the release commit; the tag and the GitHub release exist, with the
  Windows programs.
- **By hand.** In the Windows virtual machine: *Hunna Wood*'s editor, its dialogs, its viewport,
  Play, and the asset browser.

### What the exit needs at minimum

If the stage must be cut, the exit needs M6c.1 to M6c.3, M6c.5 and M6c.6, with prefabs placed from
a list. It can do without panels docked by drag (M6c.8), thumbnails (a list of names serving),
stamp mode, several viewports, a view of the running game and "keep this value", the notify watch,
and modal keys. Play ([ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md)) and the layout of
files ([ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md)) belong to the stage by
decision, not by the exit. What is cut is decided at
[Checkpoint A](#checkpoint-a--the-owner-tries-the-editor), which cut nothing on Linux.

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
- **G20, in the Windows virtual machine.** A source asset whose `.meta` differs from its own name
  by case only, cooked twice: whether its ID changes at each cook, and whether `slime-cook watch`
  cooks it again without end (see [Left by the audit of 0.4](#left-by-the-audit-of-04)).

## Releasing

A release is an annotated git tag on a commit whose CI run is green on Linux and Windows
([ADR-0004](adr/0004-workspace-layout-and-crate-naming.md)). Tags and GitHub releases wait for the
owner's go.

1. In one commit: set the version in the root `Cargo.toml` and the tag that the README's "Using
   SlimeEngine in a game" names, with its list of crates; rename the `[Unreleased]` section of
   `CHANGELOG.md` to `[<version>] - <date>`; update this plan and [`history.md`](history.md).
2. Push that commit and wait for its CI run to pass.
3. Tag that commit, not a later one, then push the tag:
   `git tag -a v<version> -m "SlimeEngine <version>" <commit>`.
4. Create the GitHub release of the tag, its `CHANGELOG.md` section as its notes, with the
   `slime-examples-windows` artifact of that commit's CI run attached as
   `slime-examples-windows-<version>.zip`: CI keeps its artifacts 14 days, a release keeps them.
   Beside the programs, CI puts DXC's library and its licence, and the licenses of the fonts and
   icons that they embed: parts of Noto in `interface.exe`; Geist, Geist Mono and Lucide in
   `widgets.exe`, and in the dev overlay of `scene.exe`, `world.exe` and `hunnawood.exe`. Add
   `LICENSE.txt` and a `NOTICE.txt`.

## Known debt

Limitations known and accepted for now.
Those that a contributor could take on alone are also GitHub issues, linked from their entry; this
list stays the reference.

- **What a load reads waits in staging memory until its copies are submitted**
  ([ADR-0118](adr/0118-the-task-pool-and-the-copies-to-the-gpu.md)).
  The renderer's uploader sends at most a budget of bytes per frame; nothing holds back loads that read faster.
  A load of many hundreds of megabytes keeps them in the CPU's memory while it lasts.
  The asset requests ([ADR-0128](adr/0128-asset-requests.md)) rank the loads and stop those that nothing holds, but hold none back.
  How many loads' tasks run at once, and the budget, were measured on the owner's RX 6900 XT alone.
  Each renderer has an uploader of its own: two renderers send twice the budget.
- **The development channel scripts keys only, and captures windowed runs only**
  ([ADR-0122](adr/0122-the-development-channel.md)).
  A tool puts keys down and lets them go, not the mouse nor text; an agent reaches an interface through its elements' actions instead ([ADR-0143](adr/0143-what-agents-see-of-the-interface.md)).
  A run without a window has no image to capture.
  The channel's measurements are the frames' times and the reports that a game gives, its views' statistics among them.
  A component that refers to an entity without a persistent identity is read with the reference written as no entity, `""`, and written back so.
  A window's image is created with the copy-source usage wherever the system offers it, at every frame, captured or not; its cost to a driver is not measured.
  Each later milestone exposes its system to the channel ([ADR-0116](adr/0116-stage-6-in-two-halves.md)).
- **The first frame that draws shadows costs more, and more still while a scene loads**
  ([ADR-0074](adr/0074-uploading-textures-without-holding-up-frames.md)).
  It makes the renderer create its shadow atlas and its frame again.
  During a load, the GPU, busy with the load's copies, presents that frame late: past 33 ms in some runs, measured before M6a.2.
  M6a.2 holds the copies to a budget once a frame is drawn, but not those before the first frame, which that frame waits for; not measured again.
  *Hunna Wood* draws its hero, and so its first shadows, from its first frame.
  Creating the atlases with the renderer would remove it, at a cost in memory for every program, shadows or not.
- **The zone costs up to 8 s per frame on lavapipe**, the CPU driver of the checks; the RX 6900 XT takes under 2 ms.
  Most of it goes to the sun's four shadow cascades, each 2048 × 2048 texels, through which thousands of overlapping leaf cards are drawn with a shader that discards.
  The checks run the game for one frame by day, one at night, two around a lost device and one resumed from the save fixture.
  The oaks' cards, made smaller and four times as many at M4b.8, changed it little: the day's shadow pass took 6.0 s instead of 6.2 s, in a release build.
  Leaves that cast no shadow in the farther cascades would lower it; to measure with views of the zone.
- **`cargo test` leaves out the tests of the cooked zone**: they need the zone fetched and cooked, so they are ignored (`#[ignore]`).
  `scripts/check.sh` and CI's `windows` job fetch and cook the zone, then run them (`cargo test -p hunnawood -p hunnawood-editor -- --ignored`).
  The agents' sessions through the MCP server, the session tests of `slime-mcp`, are tests apart (`test = false`), which `scripts/check.sh` runs on Linux alone.
- **Frames are not synchronized with the display's refresh** (ADR-0021; issue
  [#1](https://github.com/HoneySlatt/SlimeEngine/issues/1)).
  With Mailbox, the default, the schedule alone paces frames.
  On a 60 Hz display, an image is now and then shown twice while the next one is dropped.
  On a display of 144 or 165 Hz, the default 60 frames a second last 2 or 3 refreshes in turn, which shows as judder in motion.
  A game can raise `AppConfig::max_frame_rate` to the display's rate, which the engine does not report, or choose `PresentMode::Fifo`, which shows one image per refresh but can stall (next entry).
  Aligning frames on the compositor's frame callbacks is to reconsider when a renderer draws real scenes.
- **Without Mailbox, presenting can still stall the loop.**
  Windows that do not offer it, and applications that choose `PresentMode::Fifo`, wait for the display: one frame per second for a hidden window on niri.
  In a headless Weston with lavapipe, `acquire` waits for Weston's frame callbacks for a time that depends on the machine's load.
  120 frames of the `window` example took 15 s on 2026-10-12 (release build, waits of 130 ms on average), 9 s and 25 s in other measurements that day, against 2 s with Mailbox.
- **A hidden window keeps drawing** at the scheduled rate (2 to 3 % of one CPU core on the development machine).
  A program run on demand, as the editor (`AppConfig::on_demand`), draws only when woken.
  The engine cannot tell on Wayland whether its window is visible.
  winit reports occlusion on X11, which the engine ignores: under a compositing manager, Xwayland included, the X server reports windows as visible anyway.
  On Windows, a window covered by another keeps drawing too; a minimized one, of size zero, draws nothing (M3b.9).
- **wgpu's errors panic, in release builds too** (ADR-0016): a use of wgpu that it rejects is treated as a bug.
  Before games ship, logging the error and dropping the frame is to be considered.
  Running out of GPU memory is not such a use: creations return `GfxError::OutOfMemory`; memory that runs out where no caller hears of it gives the device up, which stops the run (ADR-0044).
  The engine avoids the rejections that the environment can cause.
  A window larger than the device's largest image is drawn at that size (`Surface::resize`).
  A shader or pipeline that the driver rejects is returned as an error: `GfxError::Shader` or `GfxError::Pipeline` on Direct3D 12; on Vulkan, wgpu treats the driver's failure as a lost device, `GfxError::DeviceLost`.
  A window's surface that the graphics API refuses to configure counts as lost (ADR-0025).
  The refusal of a configuration is not tested: nothing the tests control makes a conformant driver refuse one.
- **On Linux, a game goes on after a real reset of the GPU only if the compositor does**
  (ADR-0075).
  A reset reaches every program that uses the GPU.
  Mesa ends the programs that did not ask to be told of it, as niri, Hyprland and COSMIC do not.
  On 2026-10-15, programs that hung the owner's RX 6900 XT made the amdgpu driver reset it twice; niri ended each time, taking the session with it.
  KDE Plasma's KWin recovers from resets: on 2026-10-16, after a reset forced in a Plasma session, *Hunna Wood* opened a new device 70 ms after it found the loss and drew its zone again 2 s later, its run kept.
  The terminal it was started from and Plasma's panel ended.
  On Windows, the desktop's compositor survives resets: the game went on after a forced timeout detection and recovery (TDR) in the owner's virtual machine.
- **A lost GPU device is replaced only for the applications that ask for it** (ADR-0075; issue
  [#2](https://github.com/HoneySlatt/SlimeEngine/issues/2)).
  The others stop with an error; since winit allows one event loop per process, carrying on means starting the process again.
  An application that asks drops and creates its GPU objects again itself, as *Hunna Wood* does.
  A real driver reset can only be checked by hand, as it was at Stage 4b's end (above).
  A restore that fails in a windowed run is not tested: nothing makes one fail on purpose there.
  So nothing tests that `run` then returns the application's error first (ADR-0103).
- **Three behaviours of wgpu 30 are worked around, and not reported to its authors**
  ([ADR-0103](adr/0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md)).
  wgpu holds a device invalid before it reports the loss, without a way to ask, and reports the use of objects created in between as a misuse.
  `Device::poll` panics on a lost device.
  On Vulkan, wgpu panics when it destroys a swap chain whose image work in flight still holds.
  The engine asks with a buffer of 4 bytes after each creation.
  It waits for the GPU by submitting empty work, which wakes later after the GPU finishes than a poll would (`Gpu::wait_idle` says how much).
  It releases a surface's last image before its swap chain.
  P3 asks the owner's agreement to report them.
- **In capture tools, the sky pass shows as a second `scene`.**
  A pass is named after the target it draws into; the sky pass draws into the scene's high-dynamic-range target, after the scene pass (`declare_frame` in `crates/slime-render/src/renderer.rs`).
  Only its pipeline is named `sky`.
  Seen in RenderDoc on Direct3D 12 (M3b.9).
- **Frames pause on Windows while the user moves the window, holds its title bar or a border, or
  opens the system menu**.
  The system then runs a loop of its own, in which no redraw arrives and the engine's loop does not wake.
  Frames and the check for termination signals pause until it ends (ADR-0020; the move was measured in M2.4).
  Keeping frames going would mean asking for redraws from inside each frame and waiting there for the schedule.
- **wgpu-hal logs an error when an adapter cannot present to the window**, even though the engine then uses another one.
  Seen with RADV in a headless Weston, where only lavapipe can present.
- **No title text in client-side decorations.**
  On compositors that do not draw decorations (GNOME), the title bar has no text: winit's text renderer depends on an unmaintained crate (ADR-0011).
- **Input not reported yet** (issue [#3](https://github.com/HoneySlatt/SlimeEngine/issues/3)):
  touch, touchpad gestures, gamepads, dropped files.
  The window cannot go fullscreen.
  On Windows, winit 0.30 reports as raw motion the positions that a virtual machine's tablet or a remote desktop gives ([ADR-0157](adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md)).
  M6c.7 brings files dropped on Windows and X11.
  Stage 6d brings gamepads and fullscreen ([ADR-0170](adr/0170-applications-on-the-ui-framework.md)).
  Files dropped on Wayland wait for winit 0.31 ([ADR-0155](adr/0155-the-editors-design.md)).
- **A compositor that stops answering blocks the engine.**
  Before the first frame, winit waits for the window's first configuration without a timeout: a termination signal is noticed only when a second one ends the process.
  After the `stop` methods, releasing the window or the surface can block too (seen with a Weston stopped by SIGSTOP).
- **Logging off or shutting down Windows ends a game without its `stop` methods.**
  Windows tells a window of it with end-session messages, which winit does not report.
  It sends no console event to a process that has loaded `user32.dll`, as every engine process has (`crates/slime-app/src/signal.rs`).
  What a game saves in its simulation's `stop` is lost then.
  *Hunna Wood* saves its run there since M5a.9: the run since the last F5 is lost when Windows ends the session (ADR-0088).
  A fix needs a new dependency, which ADR-0081 leaves to a later decision.
- **WARP's shader compiler crashes the process, now and then, when several threads compile
  shaders at once**, even on separate devices.
  It is an access violation (`STATUS_ACCESS_VIOLATION`) that other projects trace to `d3d10warp.dll`, and that no test can catch.
  Seen once in the Windows CI job, on 2026-10-13, in `slime-gfx`'s texture tests, after about ten green runs.
  Other projects that test on WARP report the same crash, and that running the tests one at a time avoids it.
  The Windows job does so since, and `CONTRIBUTING.md` asks it of contributors on WARP.
  The engine's own code is not involved.
- **Client-side decorations run `dbus-send` twice when the window is created** (winit's sctk-adwaita, on compositors that do not draw decorations).
  Each waits up to 100 ms when the settings portal does not answer.
  The error they log when no session bus or portal answers is hidden by the default log filter (ADR-0019).
- **Mipmaps of textures made in code are computed on the CPU when they are created**
  (`Gpu::create_texture`, whose documentation gives the cost).
  The work runs on the calling thread, in proportion to the texture's pixels (ADR-0032): a level of tens of large textures waits seconds.
  A caller with levels of its own gives them to `Gpu::create_texture_with_levels` (ADR-0039), as cooked textures do, whose levels the cooker computed (ADR-0071).
- **Cooked bytes are the same from one cook to the next on one platform, not across platforms**
  (ADR-0071).
  The cooker computes the sRGB curve with the platform's mathematical functions, which may round differently on Linux and Windows.
  Tests compare decoded values across platforms.
  A table of the curve's values would make the bytes the same everywhere.
- **A cook reads and hashes every source file at every run, and reads every cooked file it
  reuses**, and cooked files that no source makes any more stay in the output directory (ADR-0071, ADR-0102).
  Nothing records a source's size and time to skip unchanged ones; to measure on *Hunna Wood*'s zone.
  Nothing cleans the output: each change that `slime-cook watch` cooks leaves the files it replaced (ADR-0076).
  The rustdoc of `slime_cook::cook` measures what reading the cooked files costs.
- **A cooked file damaged within its ranges is not found** (ADR-0102).
  The cooker cooks again a file deleted, emptied, cut short or that its reader refuses, and a game refuses such a file with an error.
  Texels or numbers changed within their ranges draw wrong, without a word.
  Checksums may come beside the rule, with archives (ADR-0096).
- **Two watches repeat the same polling** (ADR-0077).
  `slime_assets::ManifestWatch` and `slime_render::ShaderWatch` each read their files every half second and tell a failure once, with the same dozen lines.
  No crate that both reach suits a type for both yet.
- **Every offscreen render target and depth target can be sampled** (ADR-0033): they all carry the usage that allows it, which may cost some compression on some GPUs.
  Measured at M3b.5 on an RX 6900 XT (RADV), with two builds that differ only by it: no cost (the scene pass took 43.9 µs with it and 44.0 µs without at 1920 × 1080, ADR-0033's correction).
  Other GPUs are not measured.
- **A pipeline that does not fit its pass, or a target sampled by the pass that draws into it,
  panics only when the commands of its submission are finished** inside `Gpu::submit` (or `Gpu::render`, `Gpu::render_depth`).
  wgpu's message names the pass, not the faulty call (ADR-0033, ADR-0041).
  For the renderer's frame, the render graph refuses a pass that samples what it draws into before wgpu sees it.
- **On X11, losing the connection to the X server ends the process** with exit status 1, inside the X11 library (Xlib).
  `EventHandler::exiting`, the `stop` methods and the return of `run` do not happen, although ADR-0010 promises them once the `start` methods have run.
  winit installs no IO error handler.
  Installing one (`XSetIOErrorExitHandler`, libX11 1.7 or later) would need a new direct dependency such as `x11-dl`.
  On Wayland, a compositor that dies ends the loop cleanly.
- **Without libxkbcommon, the windowing library panics** rather than returning `PlatformError::Connect`.
  On X11 it panics at once, also without libxkbcommon-x11 or without the X server's XKB extension.
  On Wayland it panics when the compositor first reports a keyboard, which can be in the middle of a run.
  Turning it into an error would mean probing the libraries before winit does.
- **A termination signal received after `run` has returned no longer ends the process by itself**
  (ADR-0012).
  `ctrlc` cannot remove the handler that `run` installs.
  A first signal received then only stops later runs, and logs that the run stops after the current frame; only a second one ends the process, with exit code 130.
  This matters for a program that keeps working after `run`, under a service manager for example, which waits out its stop timeout.
  Ending the process at once when no run is active would amend ADR-0012.
- **A window frame dropped without being presented keeps one of the window's images on Vulkan.**
  wgpu 30 discards such a frame with wgpu-hal's `discard_texture`, which does nothing on Vulkan.
  The image stays acquired until the surface is configured again; once none is left, `Surface::acquire` waits one second and returns `None`.
  `SurfaceFrame` documents this as a misuse, and nothing reports it.
  The engine presents every frame it acquires, but for a panic in `Presentation::render`.
  The surface releases that frame when it is dropped, as it releases an image that a lost device refused to present ([ADR-0103](adr/0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md)).
  A `Drop` that presents was rejected: it would show a black or half-drawn frame instead of skipping it, and present during unwinding.
- **An oversize window's surface is configured again at every frame on X11.**
  A window wider or taller than the device's largest image (the adapter's, 16 384 pixels on lavapipe and the RX 6900 XT, since M6a.4 asks for the adapter's limits) gets images of that size, smaller than the window.
  On X11, Mesa's Vulkan drivers mark such a swapchain suboptimal, and every image of it too.
  `Surface::acquire` treats each suboptimal image as a reason to configure the surface again, at the same clamped size: every frame waits for the GPU and rebuilds the window's images.
  The frames are correct, and only `debug` logs show it; Wayland and Direct3D 12 are not affected.
  Not setting `stale` in the `Suboptimal` arm of `Surface::next_image` while `oversize` is set is not enough: measured, the `Outdated` branch then configures the surface again.
  Such windows are rare since the device asks for the adapter's largest image, and were not measured again.
  To measure, before and after: start Xvfb with a screen wider than 16 384 pixels.
  With `WAYLAND_DISPLAY` unset, `DISPLAY` set to it and lavapipe, run the child of the `oversize` scenario of `crates/slime-app/tests/windowed_lifecycle.rs`, its window widened past the largest image, with `SLIME_LOG=wgpu_core=debug`.
  Count wgpu-core's `configuring surface with` records (about one per frame without the fix, one with it) and note the frame times.
- **Termination signals are ignored for the rest of the process when `ctrlc` cannot start its
  thread.**
  `ctrlc` installs its handlers, then starts the thread that waits for what they catch.
  When that thread cannot start (a limit on threads reached when `run` installs the handler), the handlers stay installed and SIGINT, SIGTERM and SIGHUP (Ctrl+C and Ctrl+Break on Windows) are ignored.
  Only SIGKILL, or closing the console on Windows, ends the process.
  The engine logs a warning that says so and gives the system's error.
  Restoring the default dispositions would need a direct dependency on libc or nix.
- **Any `trace` directive in `SLIME_LOG` makes wgpu build its per-command trace records.**
  tracing-subscriber sets the `log` crate's maximum level from the most verbose directive.
  With `slime_app=trace`, wgpu-core then formats and filters out about five records per draw while it encodes a pass, counted in the caller's span (`render` in a windowed run).
  Measured with `RenderStats::cpu_time` for a scene of 1000 instances: 0.38 ms more per frame, 35 %, than with `SLIME_LOG` unset.
  Alternatively, profile with `perf record -g` and look for `tracing_log::LogTracer::log`, `EnvFilter::enabled` and `wgpu_core::resource::Labeled::error_ident`.
  Capping the `log` level needs `log` or `tracing-log` as a direct dependency (ADR-0006).
  Span close lines are also written while the enclosing span is entered, so `frame`, `render` and, with `slime_gfx=trace`, `RenderStats::cpu_time` include them: take timing baselines with `SLIME_LOG` unset.
- **On Windows with Vulkan (`WGPU_BACKEND=vulkan`), an acquire that waits more than one second
  loses the device.**
  wgpu-hal 30.0.1 waits on a fence after `vkAcquireNextImageKHR` hands out an image, on Windows only, and maps a timeout of that wait to `DeviceError::Unexpected`, which wgpu-core treats as a lost device.
  A GPU frame longer than about one second, or a compositor that holds the window's images that long (a covered window, a locked session), then stops the run with `GfxError::DeviceLost`, with a reason that blames the driver.
  On Linux the frame is skipped instead.
  Not reproduced: in the owner's Windows 11 virtual machine with an AMD driver, the `scene` example kept running with its window covered for six seconds and with the session locked (2026-10-13).
  To report to wgpu: that wait's timeout should give `SurfaceError::Timeout`, as its other waits do.
- **Seen from below, the sample scene's walls and crates flicker where they touch the floor.**
  Their bottom faces lie in the floor's plane; since M3b.3 the floor is double-sided, so from below both are drawn at the same depth (z-fighting).
  The `scene` example keeps its camera above the floor, but another viewer of the asset sees it.
  Lifting them by a few millimetres in `examples/assets/make_scene.py` fixes it, at the cost of rendering the glTF references again.
- **A mouse button can stay down on Wayland when the compositor takes the pointer away during a
  press without taking the keyboard focus.**
  winit 0.30 reports only `CursorLeft` then, and `Translator` (`crates/slime-platform/src/event.rs`) releases held buttons only when the focus is lost.
  Releasing them on `CursorLeft` would be right on Wayland only: on X11 and Windows the cursor leaves the window during an ordinary drag.
- **On Windows, the first of two held Shift keys let go of is released late.**
  Windows sends a single key-up for both Shift keys, so the engine releases the first one only when the second goes up.
  Windows 11 marks the second Shift's press as a repeat, so neither its press nor its release is reported (seen in the owner's virtual machine on 2026-10-13).
  Polling `GetKeyState`, as GLFW and SDL do, would remove the delay and report both keys, but needs `windows-sys` as a direct dependency.
- **After a dead key, a press and its release report different logical keys.**
  On Wayland and X11, a character key that does not combine with a pending dead key is reported as `Key::Unidentified` with no text.
  When the keys combine, the press is the composed character and the release the plain one.
  winit exposes no better value; rebuilding one would need libxkbcommon called directly.
  Documented in `KeyEvent::logical`.
- **A key pressed just after the window gains the focus can be swallowed on X11 and Windows.**
  winit reads which keys are down when it handles the focus event, not when the focus changed.
  A key pressed in between counts as held before the focus, and is not reported until it is pressed again.
  On X11 the gap covers the `start` methods when the window manager focuses the window while they run.
  The translator cannot tell such a press from X11's unmarked first auto-repeat.
  On Windows, trusting `KeyEvent::repeat` would close the gap, at the cost of a rule that differs between platforms; left as documented.
- **No test covers the empty submission of frames without a window image.**
  When `Surface::acquire` gives no image, it submits an empty batch, so that wgpu releases the writes a game makes outside `Presentation::render`.
  Without it, 2000 frames of a surface resized to zero, each writing 64 KiB to a uniform buffer, grew the process by about 120 MiB (lavapipe in a headless Weston, 2026-10-13), measured by a program kept out of the repository.
  The `draws-nothing` scenario of `crates/slime-app/tests/windowed_lifecycle.rs` checks the same for frames whose image nothing draws, not for frames without an image.
- **A GPU time can read 0 once when the timestamp counter wraps.**
  On adapters whose timestamps have 36 valid bits (Intel's Vulkan driver, about once every one to one and a half hours), a timed pass that runs across the wrap ends before it starts.
  `GpuTimer::latest` then gives 0 for that pass until the next time replaces it.
  wgpu does not tell how many bits are valid, so the wrapped difference cannot be recovered.
  Dropping such a sample (`end.checked_sub(start)` in `crates/slime-gfx/src/timer.rs`) is a one-line change if it ever matters.
- **Exposed light beyond 64 × 65 504 is clamped** (ADR-0035, ADR-0037, ADR-0039, ADR-0040).
  The scene and sky shaders write exposed light divided by 64 into the `Rgba16Float` target.
  They write light above 64 × 65 504 as 65 504, the largest half, rather than leave it to the driver, which may store an infinity that the tone curve turns into NaN.
  lavapipe and RADV clamp it themselves, so no test can show either clamp.
  The sun's disc, 1.47 × 10⁹ nits for the sample's sun, is clamped from an exposure of 2.85 × 10⁻³, six and a half stops above the sample's.
  Emission above 64 × 65 504 / exposure, and any light once the exposure is raised about 13.5 stops above the sample's, are clamped too; the bloom spreads them with the energy they keep.
  A sky's radiance is at most 65 504 nits before the exposure: brighter zenith, horizon or ground is refused, and a brighter glow kept within it.
  Divided by 64, exposed light below 64 × 2⁻¹⁴, about 0.004, is stored in half precision's subnormals, in steps of 64 × 2⁻²⁴.
  That is black on an 8-bit display, whose darkest step is about 0.007 exposed; an output of higher precision would show its steps.
- **Every fragment evaluates every light of the scene** (ADR-0037).
  The scene pass has no light culling: its cost grows with the number of lights.
  About 20 µs per light for a screen of 1920 × 1080 fragments on an RX 6900 XT (2026-10-13), 338 µs for 16, the most a scene holds, and 10 to 20 times more on an integrated GPU.
  A light's range saves no work.
  More lights would need clustered or tiled culling and a storage buffer, a measured decision of Stage 9a (ADR-0052).
- **Point lights cast no shadows, and light what objects hide** (ADR-0037).
  The first directional light that casts shadows casts them (ADR-0038), and so do spot lights (ADR-0058).
  The sample's ring light, a point light, reaches 4 m, and gives little beyond a metre.
  The shadows of point lights come with Stage 9a (ADR-0052).
- **Every instance of a scene costs CPU time at each frame** (`Renderer`,
  [ADR-0117](adr/0117-frustum-culling-and-instancing.md)).
  Frustum culling and instancing make a frame's draws and its GPU work follow what each pass sees, but the renderer keeps nothing from one frame to the next.
  At each frame, on one thread, it places and tests every instance's box against the camera's view and against every cascade and spot light, ranks every instance's material and mesh, sorts those drawn, and packs and sends their values.
  That is about 0.2 µs per instance of 100 000 cubes on the owner's machine (`render.instances.100k`, [`measurements.md`](measurements.md)).
  A render scene kept from one frame to the next, with stable keys, and culling and draws decided by the GPU come with Stage 9a (ADR-0097).
- **The tests that run out of GPU memory under a budget depend on the machine's memory in use**
  (`slime_gfx::MEMORY_BUDGET_ENV_VAR`).
  On lavapipe, the budget is a share of the memory that the whole machine uses.
  The shares at which such a test runs out of memory as it means to lie in a window a few shares wide, which what else runs moves by a share or two between runs.
  On 2026-10-22, `a_view_whose_new_size_does_not_fit_keeps_no_targets` failed twice locally and once in CI, run at the window's lower end.
  These tests now run halfway through the window, which probes find first (`slime-gfx`'s `tests/common/memory_budget.rs`), and that one alone in its binary, `slime-render`'s `tests/oversize.rs`.
  Each probe of that test makes a renderer: about 15 s on lavapipe, a minute or more on WARP.
  Memory that moves by a third of the window while one runs still fails it.
- **A picking draws all that its view sees, and picking by the CPU ignores masks**
  ([ADR-0130](adr/0130-picking.md)).
  A picking's pass draws every instance that the view's scene pass draws, not those of its area alone, in the frame that it is drawn.
  A view draws one picking per frame.
  Picking by the CPU passes through nothing that a mask cuts out.
  It reads each mesh whole at its first cast in a world that names it, to know its box; a bounds-only read of the cooked data would spare it.
  It reads from the cooked data that its `Picker` was made with, not again when that changes.
  Lines are not picked.
  *Hunna Wood* does not pick yet: a tool's picking in it is refused.
  Scripted input has no mouse: an agent picks through the MCP server's `pick` tool, not by clicking.
- **What a client draws into a view is lines and instances, in the scene and over the image**
  ([ADR-0129](adr/0129-drawing-for-the-editor.md),
  [ADR-0161](adr/0161-the-open-step-and-the-handles.md)), and **an outline around instances**
  ([ADR-0159](adr/0159-picking-and-the-selection.md)), for which a frame resolves the scene's depth.
  Over the image, the unlit kind's instances alone are drawn, the others left out without a warning.
  They are drawn with a depth target as large as the view's output, whichever rectangle the view draws into.
  The outline's cost is not measured at 3840 × 2160 with many entities selected: at its widest, each pixel of the view reads its mask 289 times.
  A line that lies on a surface may be hidden by it in places.
  Blended surfaces are drawn in no particular order until Stage 9a sorts them.
  The `world` example selects with Tab from the keyboard alone: scripted keys reach a game's input, which its simulation reads, not its presentation's events.
  How long `Renderer::set_shaders` takes was measured before the unlit kind's pipelines and the lines'.
- **A page of meshes keeps the ranges of dropped meshes until new ones fill them or it empties**
  ([ADR-0117](adr/0117-frustum-culling-and-instancing.md)).
  Nothing moves a mesh to pack its page: a game that drops half its meshes keeps their pages' memory while one mesh stays in each.
  Dropping a mesh frees no GPU memory until its page empties.
  The asset cache counts its pages, and a held mesh keeps the releasable meshes of its page ([ADR-0128](adr/0128-asset-requests.md)).
  Capture tools name a page's buffers, not its meshes.
- **The asset cache's requests leave work and memory behind**
  ([ADR-0128](adr/0128-asset-requests.md)).
  Two batches loading at once that both read a texture the cache does not hold yet upload it each, and the cache keeps the first.
  The copies that a stopped load handed to the uploader still go to the GPU, at its rank.
  A texture is dropped whole, its finest levels with its coarsest, until textures stream by their levels at Stage 10a ([ADR-0113](adr/0113-asset-requests-residency-and-priorities.md)).
  *Hunna Wood* has a frame builder for each time of day, each with its sky's environment; the one that does not draw keeps the handles of its last frame until it draws again.
  The tests drop meshes from one page only, which the squares that they load share: no test drops the least recently held of several.
- **Cascades are fitted to spheres around slices of the view** (ADR-0064).
  Each covers its slice whichever way the camera looks, which keeps its texels steady, but a wide view makes the spheres large.
  In the first game, *Nightwatch* (deleted at M4a.7), with the single map's 30 m, the texels near the player would be about half as wide as its texels, not a quarter, by ADR-0064's study.
  That game's moon cast its shadows up to 16 m to make them finer.
  Fitting the cascades to what the camera sees at each frame, or virtual shadow maps, is left to Stage 9a.
- **Shadows shift, and thin objects close to a surface lose theirs (peter-panning)** (ADR-0050).
  The normal offset that keeps surfaces from shadowing themselves looks a point up 4.5 texels × the sine of the light's incidence off its triangle (ADR-0063, ADR-0065).
  At the default distance, under a light 45° from the normal, that is 1.2 cm in the first cascade and 18 cm in the last, and 26 cm there at grazing.
  A shadow shifts by up to that much, and a sheet lying closer than that above a floor casts nothing on it.
- **Every draw of the scene pass runs a shader that can discard fragments** (ADR-0035).
  The `MASK` alpha mode discards in the one fragment shader that every material uses, opaque materials with a cutoff of 0.
  Some GPUs then test depth after the shader rather than before, for opaque draws too, which costs overdraw.
  On lavapipe, opaque materials drawn without `discard` did not make the scene pass faster ([ADR-0106](adr/0106-the-measured-quick-wins.md)); on a GPU, not measured.
  To measure: compare the scene pass's GPU time on a scene with much overdraw against a build whose opaque materials use a fragment entry point without `discard` (six pipelines instead of three).
- **Edges that a masked material cuts inside a triangle stay aliased** (ADR-0059).
  Multisampling smooths the edges of triangles, but the scene shader runs once per pixel, and the `MASK` alpha mode keeps or discards all of a pixel's samples at once.
  Alpha to coverage, which would smooth those edges, behaves differently from one driver to the next, which image tests would see.
  The leaf cards of *Hunna Wood*'s trees and its ferns are masked.
- **The sky's light is neither shadowed nor occluded** (ADR-0039).
  A surface under an object, or inside a box, receives the whole sky, but for its material's occlusion texture.
  No ambient occlusion (screen-space effects are outside the stage), no reflection probes; mirrors reflect the sky, never the objects around them.
  An interior is lit by the whole sky through its walls: until then, draw it without an environment, or with a sky of lower radiances, and light it with point and spot lights.
  Khronos's Sponza shows it (M3b.9): its arcades are lit by the sky as its open courtyard is, in the same blue.
- **Glass is drawn opaque.**
  The importer leaves out glTF's `KHR_materials_transmission` and `KHR_materials_volume`, and warns that it does.
  The renderer neither lets light through a surface nor blends one (Khronos's FlightHelmet and ABeautifulGame, M3b.9).
  Transparency is left to Stage 9a (ADR-0052).
- **One GPU, an AMD, and two CPU drivers have run the engine.**
  The image tests' tolerances (ADR-0030), the measurements of this plan and the samples' looks come from an RX 6900 XT, with RADV on Linux and AMD's driver on Windows, from lavapipe and from WARP.
  NVIDIA's and Intel's GPUs are untried, and stay so for now, the owner having no other machine (ADR-0097).
  The ratios that scale the hardware tiers' budgets to the RX 6900 XT are estimates from public benchmarks until the tiers' machines are measured ([ADR-0114](adr/0114-hardware-tiers-and-the-gpus-limits.md)).
  A GPU rented in the cloud, a Steam Deck, or contributors would end it.
  The handheld's ratio, a tenth, may be generous for the interface's canvas, whose cost lies more in writing pixels than in computing them: the Steam Deck writes them about twenty times slower than the RX 6900 XT ([ADR-0137](adr/0137-the-canvas.md)).
  The RX 6900 XT also draws the owner's desktop, whose work moves its figures ([`benches/README.md`](../benches/README.md#reading-the-figures)).
- **The environment is computed on the CPU, in every process that draws a sky** (ADR-0039).
  It takes 23 ms in a release build on the owner's 16 threads, 162 ms on one core, and the BRDF table once (15 ms).
  It takes about eight times longer in a debug build, which every test that draws a sky with a gradient or a sun pays.
  Most of that goes to `slime-render`'s `tests/environment.rs` and the examples' `tests/gltf.rs`, whose every load of a file computes the sample's sky.
  They took 9 s and 13 s, timed by hand at P5, of the workspace's 79 s of tests on lavapipe, on a machine of 4 logical CPUs ([measurements](measurements.md) holds the total).
  A sky that changes needs a new environment: no time of day.
  To measure: `cargo test --release -p slime-render --lib -- --ignored --nocapture`, against the budget of M3b.6.
  Not yet scheduled: to move to the cooker, or to compute passes if skies change at run time.
  If the tests' time matters first, building `slime-render` optimized in debug builds would shorten it, at the cost of stepping through it in a debugger.
- **The sky's sun is described twice in the renderer's own API** (ADR-0039).
  A game that draws with it gives the same `DirectionalLight` to `Sky::sun`, for the disc and the glow, and to `Scene::lights`, for its light and its shadows.
  Two different lights show a disc where no light comes from.
  The `scene` example takes both from the file's first directional light.
  A world gives it once ([ADR-0084](adr/0084-what-the-world-draws.md)), as *Hunna Wood*'s does.
- **A node renamed in Blender loses what saves held of it** (ADR-0079, ADR-0087).
  A scene's entity is known by its node's name: loading a save made before the rename sets aside what it held of the node, and reports it.
  *Hunna Wood*'s zone script names every node and fails when Blender numbers one, so that only a change of the script renames a node.
  The saves made before such a change set aside what they held of the node, or are saves of another layout.
  Identities that a scene file keeps for a former name wait for a game that has players (ADR-0089).
- **Some meshes of *Hunna Wood*'s zone are named with Blender's numbers** (`wooden_lantern_01.001` in the lantern, `capital.004` in the land).
  The script checks the names of its objects, not of their meshes.
  The cooker derives a mesh's ID from its name: a later change of the script that numbers a mesh otherwise changes the mesh's ID.
  Naming the meshes changes the export, and waits for the next release of the mirror.
- **Blender's exports differ from one run to the next** (G55, low): the hero's, the lantern's on a machine with another number of threads, the four textures that Blender renders, and the sample scene's sphere.
  Blender computes the tangents of a mesh of more than 10 000 faces in parallel, in an order that changes.
  Its renders write their date and duration in the PNG files' text.
  Its UV sphere orders its faces otherwise in each process.
  The hero kept on the mirror is the one the game reads.
  Running the scripts on one thread, with `--factory-startup` and a pinned version of Blender, and removing the PNG files' text, would make them reproducible.
- **A save cut short may load in part** (ADR-0087).
  A save is a document of sections: one cut at the boundary of a section is still a document, and loads what it holds without an error.
  *Hunna Wood* writes its saves whole or not at all (ADR-0088), as every game that writes saves must.
- **A save is text, of limited size** (ADR-0087, ADR-0102).
  Saving refuses a save larger, or of more values, than what `slime-serialize` reads: about 22 000 entities that have a name and a transform.
  A value is counted for each `=`, `,`, `[`, `{` and `.` of the text, a float's point included.
  Loading costs more than saving, much of it parsing the text; the rustdoc of `World::save` and `World::load` has the figures.
  *Hunna Wood*'s saves are far below the limit.
  The binary encoding exists since M5b.4, but saves stay text until a measurement asks (ADR-0089): a larger world needs saves in the binary encoding, or read in parts.
- **An entity that a run created, wrote into its level and destroyed since comes back when the run
  is carried into the file cooked again**
  ([ADR-0124](adr/0124-driven-components-keys-and-created-entities-in-levels.md)).
  The save no longer names it, and the file holds it.
  A save that names the written identities of what the run destroyed would settle it.
  "Keep this value", a value of the running game that the editor keeps, settles it with the inspector, M6c.5f ([ADR-0155](adr/0155-the-editors-design.md)).
- **A game's preparation of a scene that a save spawns may change only the scene's entities**
  ([ADR-0125](adr/0125-saves-per-scene.md)).
  What it creates, under a key or as a placement, is the run's, which a save holds, and which loading the save creates before it prepares the scene.
  So a preparation that creates an entity under a key fails, and one that places a prefab places it twice.
  *Hunna Wood* and the examples prepare without creating.
- **What a run created under a scene's entities outlives the scene when it is unloaded**
  ([ADR-0125](adr/0125-saves-per-scene.md)).
  It is taken out of them, at the world's root, rather than kept with what the world keeps of the scene, as an item dropped in a level would be.
- **What cannot be applied of what the world kept of a scene unloaded is logged, not reported**
  (`World::settle`).
  As when the scene changed since it was unloaded: it is logged at the `warn` level, where a save's load reports it to the game (`Loaded::set_aside`).
- **`Range::contains` stays public beside `Range::holds`**: two ways to check a number against a range.
  The first works in `f64`, and only tests' helpers use it.
  Every check of a field's range uses `Range::holds` (ADR-0101); the helpers can move to it, and `contains` go.
- **Files stay beside *Hunna Wood*'s save**: `save.toml.lock`, which locks the save while a copy of the game runs, and `save.toml.<process id>.new`, when the game is killed while it writes.
  Neither is read again, nor removed.
- ***Hunna Wood*'s lock holds a save by its path** (ADR-0101).
  Two copies that reach one save by two paths, one through a symbolic link, take two lock files and do not exclude each other.
  On a file system without locks, the game neither resumes nor saves, and says so.
  A copy of 0.4, which takes no lock, still writes a save that a later copy holds.
- **A save that removes a component of a type that the program does not know sets it aside**
  (ADR-0101).
  `removed.components` names it, and loading reports that the entity holds no such component, where a value of an unknown type is a warning.
  Writing the save back loses the removal, which a later version that knows the type would have applied.
  *Hunna Wood* sets aside the saves of later versions by the version that they say, which covers it for the game.
- **Children and queries in another order after loading** (ADR-0087).
  A world loaded from a save holds its entities in other slots, and an entity's children in the order the save set them.
  What must go the same way after loading cannot depend on either order.
- **Spawning a large scene makes each component from its value** (ADR-0091 holds the figures).
  On the game's zone, at M5b.6, reading and resolving its scenes took 1.24 to 1.75 ms in a release build, twice what 0.3's one scene took.
  A scene resolves and spawns far slower than as many entities spawned with `World::spawn` (ADR-0083): each component is made from its value, and each entity's identity is a list of texts, held twice.
  ADR-0089 orders the remedies, inside the world: typed components copied rather than made from values, identities that share their parts, scenes prepared on another thread.
- **A world written back writes what it holds of what spawning set aside** (ADR-0092).
  A reference, or a parent, that a scene file wrote into a placement whose prefab no longer had its entity is written as the world holds it, no entity or the root: what it named is lost, as a save loses it.
  An entity of a placement moved out of the place that its prefab gives it is written in that place.
  The children of a placement's entity are written in its prefab's order, then the file's.
  Both are reported: a scene file has no parents for a placement's entities, nor an order among them.
  M6c.6 decides whether a file says them, with the new layout of scene files ([ADR-0155](adr/0155-the-editors-design.md)).
- **`World::write_scene` drops a scene file's comments** (ADR-0089): it writes the canonical form.
  The editor puts them back (`slime_serialize::keep_comments`, [ADR-0134](adr/0134-the-editors-core.md)), within the limits that the editor's entries below give.
- **The cooked glTF scene is a format apart from the binary encoding** (ADR-0089, ADR-0095).
  A glTF file's scenes are cooked into a format of their own, `slime_assets::CookedScene`, and only the typed properties of their nodes into the binary encoding: cooked scenes have two formats, and two readers.
  The renderer's scene loader, the `scene` example and the image tests read the format.
  Moving it onto the binary encoding would make `slime-render` depend on serialization, and waits for a reason.
- **After a lost device, what is hidden loads only once shown** (ADR-0084): the cache loads what is drawn.
  In *Hunna Wood*, a key taken or a door opened before the loss shows a few frames after R shows them again.
- **Any change of *Hunna Wood*'s cooked data makes a new game of its scenes.**
  With `--watch`, the game makes a new game of the scenes before it knows whether a scene changed, a texture saved in an image editor included (`take_assets` in `games/hunnawood/src/main.rs`).
  It costs about 2 ms, for a tool of development.
- **A zone loaded again with `--watch` changes in parts.**
  Its entities change at once, the meshes, materials and textures that the cache holds when its reload ends, and new meshes once their batch is loaded.
  *Hunna Wood* swapped whole scenes before it ran on the world.
- **The tests that edit every leaf of every component are written twice** (ADR-0085): once in `slime-world` for its components, once in *Hunna Wood* for the game's.
  They are about fifty lines that choose another value for a leaf.
  One copy would need a test-support crate, or a feature of `slime-world` that only tests enable; neither is worth it yet.
- **An edit through reflection reads the whole component again** (ADR-0085).
  A field that its type skips takes its `Default` value.
  A rotation is edited whole, as `Transform`'s check refuses a coordinate edited alone.
  The dev overlay's editors edit a rotation, a vector and a color whole.
- **Rough metals reflect the sky with multiple scattering compensated, the lights without**
  (ADR-0039, as the Khronos glTF Sample Viewer).
  A white metal of roughness 1 reflects all of a uniform sky.
  Its lights keep glTF's single-scattering BRDF, under which it would reflect 31 % of that sky.
  Rough metals are darker in the sun than in the shade, relatively.
- **A roughness between two prefiltered levels reads a blend of their lobes** (ADR-0039).
  The four levels are 0.25 apart; a lobe in between is approximated by mixing the nearest two, the split sum's usual approximation.
  Each level is within 2.2 % of its own lobe's average.
- **The image tests use up to 75 of the 131 pixels that RADV may have off by more than 8**
  (ADR-0030, ADR-0054).
  The glTF scene's front view uses 75, on the wall's mortar joints seen at a grazing angle (76 before its textures were cooked).
  `shapes` uses 28, on edges and the metal spheres' reflections of the sky (49 at M3b.6, before the bloom; 33 before the shadow map followed the camera and the output was dithered).
  A change that adds fine or reflective detail may exceed it; measure it again then.
- **Sixteen passes per submission is a number measured on one machine**
  ([ADR-0158](adr/0158-the-viewport.md)).
  On the owner's RX 6900 XT (RADV), the memory that the CPU can map is a 256 MiB window, which the desktop's programs filled whole.
  There, a whole bloomed frame in one submission cost the CPU least, without the occasional stall in the kernel that ADR-0041 measured at M3b.8 and kept four passes against ([`docs/measurements.md`](measurements.md)).
  The owner chose sixteen on 2026-10-28.
  Direct3D 12 and other GPUs are not measured: the Windows virtual machine checks it at a checkpoint.
  A stall that came back would show as spikes in `render.instances.10k`'s 90th percentile.
- **The render graph trusts what a pass says it samples from imported targets** (ADR-0041).
  The groups that sample transient targets are built from the declarations, but the scene pass's group holding the shadow atlases is built by the renderer.
  A pass that binds an imported target must declare it, or the graph orders it without knowing.
  The views after the first of a frame read the spot lights' atlas, which the first drew, without declaring it: they rely on the order of the submissions, one view after the other ([ADR-0126](adr/0126-views-apart-from-the-renderer.md)).
- **An orthographic camera's shadows reach the sun's shadow distance from its eye**
  ([ADR-0126](adr/0126-views-apart-from-the-renderer.md)).
  A map drawn from higher above the ground than that distance shows none.
  Its four cascades are nearly as large, the image's width and height setting them more than their depths (`slime_render::Projection`).
  Fitting an orthographic camera's cascades to what it sees is to decide when a game draws such maps with shadows.
- **The views of a frame draw the same lights**
  ([ADR-0126](adr/0126-views-apart-from-the-renderer.md)).
  Beyond the 16 that the renderer draws, `slime-world-render` keeps those nearest one point that the game gives.
  Views far apart, as two players' halves of a split screen in two places, share the lamps of one of them.
  Stage 9a's method for many lights decides how a view gets its own.
- **A view whose frame changes shape at every call reports no tone-mapping or bloom time**
  (ADR-0041, ADR-0126, ADR-0127).
  Each change of shape (the size of its targets, its anti-aliasing, a pass starting or stopping, by the scene or by its settings) builds the view's frame again with new timers.
  The last submission's times come back during the next call, by then read from newer timers.
  A window's edge dragged no longer does it, its size held until it is let go.
  Nor does a view drawn into rectangles of two sizes in turn, which draws one of them stretched: use one view per size.
- **A render scale reads the scene's light bilinearly**
  ([ADR-0127](adr/0127-the-settings-of-a-view.md)).
  Below 1 the image is blurred; between 1 and 2 each pixel averages the four texels nearest its centre, missing others.
  Stage 9a's temporal upscaling is the sharper way; a pass of a sharper filter was rejected.
- **Four modes of debugging** ([ADR-0127](adr/0127-the-settings-of-a-view.md),
  [ADR-0159](adr/0159-picking-and-the-selection.md)): the normals, the shadows, the base color and the light alone.
  The editor draws the wireframe of what is selected as lines over the image, a line for each edge of its meshes, which a large mesh makes many.
  A wireframe of the whole scene waits until `PolygonMode::Line` is measured against lines along a mesh's edges ([ADR-0155](adr/0155-the-editors-design.md)).
- **Tiny, very bright sources blink in the bloom** (ADR-0040).
  Without a firefly filter, a highlight smaller than a pixel blooms as its coverage of the pixel changes from frame to frame.
  Karis's average, a firefly filter, dims the sun's halo and makes it flicker, measured.
  Temporal anti-aliasing, or a cap on the light a pixel brings to the chain, belong to Stage 9a (ADR-0052).
- **A pixel whose light is not a number spreads through the bloom** (ADR-0040).
  The renderer refuses values that are not finite in meshes and materials, and the skies' radiances.
  It leaves out a light that it cannot draw, as the frame builder does a sun, and bounds the light that each light and the sun's disc give a point (ADR-0102).
  Not everything that reaches the shaders is checked: a normal of zero length, which `MeshData` asks callers not to give, can make its pixels NaN in the high-dynamic-range target.
  So can, at an exposure of 0, an emission so strong that, added to the light of the scene, it is not finite.
  The bloom averages them into its levels, and from there into every pixel within reach of their halo: a blot where one surface was wrong.
  Dropping such light before the bloom would keep it to its pixels.
- **The bloom has one shape** (ADR-0040): every level the same weight, down to a side of 8 texels.
  No radius, threshold or tint for a game to set.
  A window resized across a power of two adds or removes a level: the glow's reach changes a little while the edge is dragged.
- **Light near the right and bottom edges blooms a little more or less** (ADR-0040).
  Where an output's side is not a multiple of 2^levels, the coarse levels' last texels straddle the edge.
  A pixel's halo there holds 0.83 to 1.37 of its light at ten sizes measured, up to 1.9 at the far corner (1023 × 1023, 2047 × 1151).
  It holds within 1.011 at 1280 × 720 and about 1.1 at 1920 × 1080.
  Inside the image and at the top and left edges, it holds its light exactly.
  To measure: the CPU model of `tests/bloom.rs` at the size in question.
  A fix would pad the chain beyond those edges.
- **glTF minification filters are approximated.**
  `slime-gfx` ties mip blending to the minification filter and gives every texture mipmaps, so only `NEAREST_MIPMAP_NEAREST` and `LINEAR_MIPMAP_LINEAR` are exact (documented in the importer, `crates/slime-cook`).
  Revisit when an engine-side user needs a separate mipmap filter or a base-level-only read.
- **A restored entity that had a base takes its values as its base**
  ([ADR-0134](adr/0134-the-editors-core.md)), as every entity that a scene spawned has.
  What a run changed of it before it was deleted is no longer told apart from what its scene gave it, and a save made afterwards does not hold it.
  The editor never saves its world; a game that deletes and restores such an entity through the world's commands loses that difference.
- **The editor's journal is handed to the system at each step, not forced to the disk**
  ([ADR-0134](adr/0134-the-editors-core.md)).
  A crash of the editor loses no step; a crash of the system may lose the last ones.
- **Undoing a deletion in the editor copies what it recorded** ([ADR-0134](adr/0134-the-editors-core.md)).
  Recording it reads every component, of every entity of the world, of a type that may refer to an entity.
  `world.undo.1k` measures the world's commands on a subtree of 1 000 entities, of types that refer to none, not the editor's step.
- **The editor does not follow the prefabs that change on disk while it edits**
  ([ADR-0134](adr/0134-the-editors-core.md)).
  Its library holds the cooked scenes as it opened them.
  M6c.6 follows them.
- **The editor's viewport shows every light that a level places**
  ([ADR-0158](adr/0158-the-viewport.md)).
  Which lights shine is a game's rules', which the editor runs none of.
  *Hunna Wood*'s zone shows its night's lanterns lit by day, and its moon, the one light in its sky, as the sky's sun.
  Its meshes come from the cooked data as the core read it when the level opened, which a level opened later reads again; a mesh cooked again while the level is open is not read again.
  The world's cameras, which the world does not have yet, have no icons.
- **Picking in the editor's viewport** ([ADR-0159](adr/0159-picking-and-the-selection.md)).
  A click and a marquee draw the whole view again for their picking, which a frame answers a frame or two later.
  Alt and a click cast a ray against meshes alone: it goes through a light's icon, which a click selects.
  What the window marks of the selection is worked out again at each frame that draws the viewport, a walk up from each instance drawn.
  The wireframe's lines, one for each edge of the meshes selected, are rebuilt at each such frame, which flying a terrain's makes dear.
  The wireframe and a ray read the meshes that they first meet from the cooked data on the window's thread, which waits for them.
  Alt and a click are told from an orbit by the mouse's moving less than 4 units of its raw motion, which a mouse of a fine resolution may pass.
- **Two viewports build their frames apart** ([ADR-0160](adr/0160-two-viewports-and-blenders-way.md)).
  They share the level's meshes, but each that draws builds the world's frame for itself: twice the CPU's work of one when both draw at a frame, as when an agent changes the world.
  Blender's way gives Blender's buttons and a few of its keys, not its modal keys, its pie menus or its numeric pad's views.
- **The editor's handles** ([ADR-0161](adr/0161-the-open-step-and-the-handles.md)).
  A drag sends a command for each entity moved at each move of the pointer, which the core undoes and applies again, and the viewport draws the level again for each.
  Nothing has measured a drag of a large selection, nor of a placement of many entities.
  Snapping is ticked in the Edit menu, not held under a key.
  In the world's axes, snapping moves the pivot onto the world's grid, never by steps from where the drag started.
  Scaling along an axis changes the stretch along the entity's own axis nearest it, which is not the handle's axis when the entity is turned in the world's axes.
  The pivot is the middle of the origins, not of the bounds.
  A drop meets meshes only, not colliders nor the ground's grid, and refuses when nothing lies below.
  A drop moves each entity on its own: two selected one above the other land as if the other did not move.
  While a person drags, an agent's requests that change the level wait; through the MCP server, which asks one request at a time, its reads wait too.
  Escape during a drag is tested through the model, not through a key of the window.
- **Creating and arranging in the editor** ([ADR-0162](adr/0162-creating-copying-and-arranging.md)).
  What is duplicated and pasted goes last among its parent's children, in place.
  A copy's root that its original named after its ID is named after its new ID, `lamp 2-r39l`.
  Nudging moves along the world's axes, whichever way the viewer looks.
  Repeating runs the last operation that made a step, not a drag of the handles.
  Grouping refuses an entity that a placement holds but for its root.
  A copy carries every component of what it creates but placements, of which it carries what differs from their prefabs, and the entities that saves leave out too.
  Pasting reads any text that the system's clipboard holds, and refuses what is not a copy or changes what it does not create.
  A copy keeps an ID's last word of four letters, and drops one that holds a digit, which the editor's IDs end with.
  So an ID of four letters that the editor made stays, `torch-abcd`, and grows a suffix at each copy.
  With a window, an agent's `edit.paste` is answered before the window pastes, at the next frame.
  A shortcut that a field of one line does not take reaches the editor while the field holds the focus: Ctrl and the up arrow in a number of the world panel nudge what is selected, and Ctrl+D duplicates it.
- **Placing prefabs and the history panel**
  ([ADR-0163](adr/0163-placing-prefabs-and-the-history-panel.md)).
  A prefab placed takes a transform of its own at its root, which replaces the turn and the scale that its prefab gives its root.
  The palette names the prefabs by their source files, without thumbnails, which wait for the asset browser.
  A prefab placed again by Repeat goes where the last one stands, on it.
  The history panel builds a row for each step that the history keeps, which a long session makes many, again as the steps change.
  Going to a step journals each undo or redo between.
- **The hierarchy** ([ADR-0167](adr/0167-the-hierarchy.md)).
  What is folded is lost when the level closes.
  A change of the world's structure makes the rows of the whole tree again, on the editor's thread.
  The search runs over the whole world at once, on the editor's thread: some 5 ms at 100 000 entities, which `editor.hierarchy.100k` measures with the rows.
  The arrows move the selection only while a row holds the focus.
  A press of a row reads Ctrl and Shift as the frame finds them held, not as the press found them: a button's press carries no modifiers.
  No double click renames; F2 does.
  No agent's session through the MCP server uses it; the window's tests read and use its elements as agents do.
- **Rearranging the hierarchy** ([ADR-0168](adr/0168-rearranging-the-hierarchy.md)).
  A drag shows where it drops, not what it carries.
  It drops on a row only, not below the last.
  It does not scroll the list as it nears its edges; the wheel does meanwhile.
  Alt and the arrows run only while a row holds the focus, as the arrows do.
  Between two steps of a game, after a change of parent or a teleport, a frame's interpolation walks each entity's ancestors again until the next step ends.
- **Regions of tabs** ([ADR-0172](adr/0172-regions-of-tabs.md)).
  The layout is lost when the editor stops, and no panel floats nor closes, until M6c.5e.
  Tabs are neither dragged nor reordered, which M6c.8 does.
  What a panel shows is built anew, its scroll at its top, when its tab is chosen again after another's.
  It is built anew too when a move changes the key of its stack or of a split around it, which the stack's first panel gives.
  So a move may build again panels that it did not move, the viewport's surfaces among them, whose gestures end.
  F6 gives the focus to a stack's first tab, not to the element that last held it there.
  From an element outside every stack, a splitter among them, F6 goes to the first stack.
  Two presses of an arrow on a splitter before the dock is built again move it once.
  A window smaller than its regions' least sizes cuts them off.
  `slime-ui` splits a box two ways: two panels and their handle, the share the interface's; and splitters, the shares the program's; the first is to be built on the second.
- **The eye and the lock** ([ADR-0169](adr/0169-the-eye-and-the-lock.md)).
  What is hidden or locked is lost when the editor stops or another level opens.
  A click on what is locked selects nothing, rather than what lies behind it, which Alt and a click reach.
  No shortcut hides or locks what is selected; the rows' buttons alone do.
  A row below a locked entity shows its own lock open, nothing marking that an ancestor locks it; a row below a hidden one reads dimmed.
  An entity selected, then hidden, stays selected, its handles drawn where it stands.
  An agent presses the buttons of a row that shows, its ancestors unfolded.
  Each change of the world's structure copies the set of what is hidden four times, for the hierarchy, the level, its frame builder and its picker: a large subtree hidden, a terrain's, makes that a cost of each step.
  A drop onto what is hidden or locked is tested through its casts alone, not end to end.
- **Play** ([ADR-0164](adr/0164-play.md)).
  A game that makes its library of scenes before `slime_app::run` starts its log, as *Hunna Wood* and the `world` example do, logs neither the scenes that Play gave nor a file of Play's directory that it left out.
  The console builds a label for each line that it keeps, 500 at most, again as lines come.
  It scrolls to the last line whenever lines come, even while the user reads older ones.
  *Hunna Wood* has no run without a window: an agent's editor without one plays it in a window all the same.
  A control that the game refuses is told, but the toolbar shows what the game says only once it answers the state asked after the control.
- **Building the game again** ([ADR-0166](adr/0166-building-the-game-again.md)).
  The game is built with its package's features alone: after a build of the whole workspace, which unifies others, cargo builds it again once.
  A change of the game's sources is seen by polling the files that cargo lists, and cargo's manifests and locks above them, until notify comes with M6c.7.
  A source that the last build did not list, a new file, is seen only with a change of one that it did.
  A new build that builds but does not start, or crashes as it starts, ends Play, and the run's save goes with Play's directory.
  Cargo stopped during a build leaves its compilers to finish the crate under way.
  On Windows, that the copy that runs leaves the build free to replace the program, and that cargo and the game open no console window, are not checked yet.
- **Play's time to control** ([`docs/measurements.md`](measurements.md), M6c.4d).
  A level of 100 000 entities gives control far over ADR-0110's budget in a debug build: the editor writes the level, and the game reads and spawns it, without optimizations.
  Such a level fits one scene file only with entities that hold little (`slime_serialize::MAX_VALUES`), which levels of several files, M6c.6, are to settle.
  The owner chose, on 2026-10-29, to leave it until M6c.6 measures it again.
  Play asks the game how its time stands at each of its frames until it gives control, which a game that holds its time long, as behind a menu, pays for meanwhile.
- **Editing while playing** ([ADR-0165](adr/0165-editing-while-playing.md)).
  An undone deletion does not reach the game, which plays on without what it restored until Play starts it again.
  A step that the game refuses stays applied in the game up to the command refused.
  A step that sets what the game's run changed overrides the run without a word.
  A gesture reaches the game once it ends, not as it drags.
- **Every multisampled depth target can be read by a shader**
  ([ADR-0159](adr/0159-picking-and-the-selection.md)), so that the outline's frames resolve the scene's depth.
  Every view of four samples makes its depth so, whether it outlines or not, which some GPUs may compress less; nothing has measured it.
- **A comment within a value written on several lines is lost** when the editor writes the level again, but for the end of the value's first line (`slime_serialize::keep_comments`).
- **The canvas draws nothing turned or scaled** ([ADR-0137](adr/0137-the-canvas.md)).
  The items' transforms, whose place they keep, come with Stage 6d ([ADR-0170](adr/0170-applications-on-the-ui-framework.md)).
  An image is read at its first level: one drawn much smaller than it is shows steps.
  An image's edges are neither rounded nor smoothed between two pixels.
  An image of colors darkens where an opaque texel meets a transparent one of another color, its texels filtered before they are multiplied by their alpha.
  A shadow is an approximation, a few hundredths of its alpha from the box blurred exactly.
  A canvas holds at most `MAX_CANVAS_ITEMS` items.
  The interface's budget is checked on the RX 6900 XT alone, through ADR-0114's ratios.
  Writing a canvas's items into its buffer costs the CPU in proportion to their number, item by item ([`docs/measurements.md`](measurements.md), M6b.3a).
- **The interface's images are textures that the program gives** for the keys of its display list
  ([ADR-0139](adr/0139-text-drawn-and-the-display-list.md)).
  Cooked images that the asset cache holds come with their first user, M6c.7's asset browser.
  A draw that asks for more glyphs than the atlas's budget holds leaves some out; a glyph larger than a page is never drawn.
  A draw that meets glyphs no draw has asked for rasterizes them on the thread that draws: a few milliseconds for a menu's first draw ([`docs/measurements.md`](measurements.md), M6b.3b).
- **The interface's text has one style per text** ([ADR-0136](adr/0136-the-interfaces-text.md)): spans of several styles in one text come with their first user.
  An italic or a weight that a family lacks is drawn with its closest face, its slant or boldness not made up.
  A face whose colors are only of `COLR`'s second version, as some emoji fonts of systems, draws no color: swash draws the first version alone.
  A word wider than its box, on a line before the last one shown, overflows the box rather than being cut.
  The cache's memory is an estimate fitted once to a measurement, of texts not cut.
  A hit copies its key, and fonts registered are never let go.
- **CI's jobs and their limit of 20 minutes** ([ADR-0123](adr/0123-the-linux-checks-in-two-jobs.md),
  [ADR-0144](adr/0144-the-cooked-zones-key-in-ci.md), [ADR-0155](adr/0155-the-editors-design.md)).
  Each job of CI must end within 20 minutes, and every session of the editor adds to the checks.
  Since M6c.0, three Linux jobs run `scripts/check.sh`'s three parts side by side; the game's agent session draws the zone without shadows.
  The cooked zone is cached, keyed on the packages that the cooker is built from: a change of the cooker's sources or of its packages cooks it again, which adds about 3 minutes to `check (game)` alone.
  On `06faca1`, at M6c.5d, the longest jobs took 16 minutes 33 seconds (`check (headless)`) and 15 minutes 25 seconds (`windows`).
  The `windows` job builds the tests that read the zone again, with `-p`, whose features differ from the workspace's: about a minute.
  Run over the workspace, `--ignored` would also run the measurements that other crates ignore, meant for release builds.
  How the jobs grew and were split: [`history.md`](history.md#ci-and-its-limit-of-20-minutes).
- **The interface's tree** ([ADR-0140](adr/0140-the-tree-of-elements.md)).
  A frame walks every element of its roots, to find what to draw again and to gather the display list, even when nothing changed.
  A list shows the rows of a new range one frame late, when it first shows or when it scrolls after its builder ran.
  A text in a root laid out from right to left keeps its own direction: a Latin paragraph starts at its box's left.
  No element is placed out of its parent's flow but in a layer of its own ([ADR-0150](adr/0150-layers.md)).
  The marks of what changed hold an element's style, layout, paint and meaning ([ADR-0146](adr/0146-screen-readers.md)).
- **Input and focus** ([ADR-0142](adr/0142-input-and-focus.md)).
  Buttons, fields and widgets alone hold the focus; buttons and widgets alone are pressed; boxes that scroll and lists scroll with the wheel.
  A press on what the interface draws is taken whether or not it does anything, from what its last frame drew: a game's interface that lets presses through draws nothing where they fall, and the wheel with it.
  The shadow that a box casts takes presses over the box, not over the blur that spreads past it.
  A root is modal from when it is added.
  The interface's actions reach no command of the editor or of a game but through the program.
  The hover follows what a layout moved under a still pointer a frame late; the focus leaves an element disabled or hidden at the next frame.
  A layout of another script than the Latin one matches a chord of a letter by the key's place, and a chord of a digit or a sign by its meaning alone.
  The example `interface` scrolls its list with time, which the wheel does not move.
  Touch comes later.
- **Text edited** ([ADR-0145](adr/0145-text-edited.md)).
  A double or triple click selects no word or line, and the caret does not blink.
  A field of one line leaves the up and down arrows to the focus, and shows no bar of its own as its text scrolls.
  It lays its text out one caret's width within its content box, so that the caret shows at either end.
  It does not scroll up and down: a text taller than its box is cut above and below.
  Escape in a field leaves the focus, which commits what the user typed rather than giving back what it held.
  Each edit lays the field's whole text out again, at a cost that grows with its length.
  A field undoes up to 100 edits, and forgets them when the program gives it another text.
  A field has no length that it holds at most, and no text hidden as a password's.
  A field of numbers has no bounds, and reads a full stop alone as its decimal mark, whatever the user's language.
  A field of numbers without a prefix is dragged only while it does not hold the focus.
  The input method's window is placed beside the caret as the last frame drew it.
  It is turned off and on again, meant to end a composition that the focus left, which no input method was checked with yet.
  A text that the program gives while an input method composes, which the field reads without the composition, is left aside: a field clears itself once the composition ends.
  The clipboard holds text alone.
  On Wayland, the clipboard needs a compositor that gives the protocol of the clipboard's managers, wlroots', niri's and Hyprland's among them, and falls back on X11 through XWayland elsewhere.
  An agent sets a field's value whole, and does not type in it key by key.
- **Widgets** ([ADR-0148](adr/0148-widgets.md)).
  A check box has no third state, mixed; M6c.5f gives it one.
  A slider lies across alone, and shows neither ticks nor its value.
  Tabs are neither closed, moved nor scrolled when the bar is too narrow for them; M6c.8 moves and closes them.
  Two panels are two, of no least size, and a third child is folded; M6c.8 gives panels a least size.
  Scroll bars show over the content as long as it does not fit, and have no arrows.
  Whether a section is open, which tab is chosen and where a handle stands are the interface's alone: they tell the program nothing, and an element that a builder removes forgets them.
  The editor's look has no italic faces, and its fonts hold no Greek.
  The look is built into the program, not cooked: the dev overlay carries it into a game's development build, and the game's build for shipping leaves it out.
- **The look's foundations** ([ADR-0149](adr/0149-the-looks-foundations.md)).
  A theme's values are typed only where a rule uses them: a name that holds a value of the wrong type is refused where a rule names it, not where the theme gives it.
  The color functions are four, and work on whole colors, not on their lightness, chroma or hue alone.
  Transitions move colors, opacity and a section's height, not sizes, places nor what a layout gives.
  They go on only while the program gives the interface its time; a section that opens is laid out twice in the frame that starts it.
  An opacity multiplies each primitive's alpha, rather than drawing the element apart and blending it whole: overlapping parts of a translucent element show through each other, until groups are drawn apart ([ADR-0171](adr/0171-effects-of-the-interface.md)).
  A shadow is not placed on whole pixels.
  On Linux, the system's dark or light preference is not read: winit reads no desktop's settings, and the portal of the desktop's settings would need a new dependency; Windows reports it.
- **Layers** ([ADR-0150](adr/0150-layers.md)).
  A menu's separators are boxes of the class `separator` that the look styles, not a kind of their own.
  A menu has no letters that jump to an item.
  A menu has no way to cross its siblings towards an open submenu: resting on another item on the way closes it.
  A list to choose from does not scroll its options: a list taller than the window is cut.
  Tab within an open list moves among its options rather than closing it.
  A tooltip shows for the keys' focus alone, not the program's, and is not placed beside the pointer.
  A floating panel has a least size of its own and no docking; M6c.8 docks floating panels.
  A floating panel does not keep its place against a window that shrinks: it is placed within the window, but keeps what the user left.
  A style sheet reads a theme's value in place of every value but a text: the look names its icons by their codepoints in its sheet, not in a theme.
  The bar of menus takes Alt let go alone whenever an interface holds one: a game whose interface holds a bar loses Alt as a key of its own.
- **Rich fields** ([ADR-0151](adr/0151-rich-widgets.md)).
  A press on a focused field's prefix that does not drag places its caret at its start.
  A field's prefix and its unit's symbol are drawn whatever their width, cut by nothing.
  A bar of progress without a share that less motion stills stays still until it is drawn again.
  The look's fallbacks for a magnifier and a cross, without a font of icons, are one disc.
  Units are of three families, lengths, angles and times, not masses, speeds or colors.
  The box of a section's header is placed by the interface after taffy lays it out, which placed it from the wrong box.
  A content box takes its padding from the left whatever the root's direction: an asymmetric padding is mirrored wrongly from right to left, which the fields' marks allow for.
- **What the editor tells and finds** ([ADR-0151](adr/0151-rich-widgets.md)).
  The palette finds commands by letters in order, without forgiving a letter typed wrong.
  The sheet of shortcuts takes no focus: a screen reader says nothing as it opens.
  A notification's mark that closes it is named `close` in English.
  No check runs a screen reader on the live regions of notifications.
  The stack of notifications turns from its corner as a layer at a point does, which holds for a stack wider and taller than its margin, as every notification is.
- **What agents see** ([ADR-0143](adr/0143-what-agents-see-of-the-interface.md)).
  A tool reads an interface only in a windowed run, from its last frame, and each ask walks the whole tree.
  A path holds while the roots below its own stay.
  A key's name that holds `/`, or that `#` and digits make, cannot be read back from a path.
  A row of a list is found by its number; the names of its texts are not gathered into its own, unless its builder names it, as the editor's hierarchy names its items.
  Games link `slime-ui` through `slime-app`, whether they have an interface or not.
- **The dev overlay** ([ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md),
  [ADR-0153](adr/0153-tab-and-f6-in-a-games-interface.md)).
  A frame after an entity is spawned or despawned makes the world panel's list again, every name read and filtered: past the budget of a list's frame at 100 000 entities.
  A frame after a rename reads again the names that changed and filters the list: under a millisecond there ([measurements](measurements.md#m6c5a-ryzen-7-5800x)).
  The frames standing still, scrolled, or after the selection changed stay within the budget.
  Its panels keep the size that they open at, their content scrolling, where egui's fitted theirs.
  It holds its interface's tables of shortcuts, which a game's interface would share.
  A game loses F6 while its interface shows a root that holds an element that can take the focus, and F6 gives a panel's bar the focus first.
  Its look, the editor's, is built into the program: its themes are files of `slime-ui`, read as the program starts, not from the disk while it runs.
  No check of CI runs a screen reader on it.
- **Screen readers** ([ADR-0146](adr/0146-screen-readers.md)).
  No check of CI runs a screen reader nor Windows' UI Automation.
  CI reads and uses the `interface` example through AT-SPI on Linux ([ADR-0147](adr/0147-the-screen-readers-check-in-ci.md)).
  The owner checks Orca and Narrator by hand, on the `interface` example and on the `world` example's dev overlay.
  A modal dialog is not marked modal to screen readers, which find the elements below it taking nothing.
  A label's text gives no runs: a screen reader reads it whole.
  A field of numbers takes no increment or decrement.
  The hover and the press are given to agents, not to screen readers, which AccessKit does not ask.
  A root's box that changes, a window resized, marks all it holds; a modal root opened or closed makes the next changes whole.
  Each element's meaning is made again from the tree when marked, the boxes that only lay others out below it walked, and a field's runs laid out from its text, as it draws it.
  On Windows, which never says that no client reads any more, a window whose tree was asked for once is given an update after each frame until it closes.
  A text that its builder names gives screen readers its name, not its words.
  A caret at the end of a line that breaks within the text is placed at the start of the next, its side lost.
  Screen readers scroll by an item or by what shows, not to an offset.
  `deny.toml` ignores three advisories of unmaintained crates that the adapter on Linux brings through zbus 5, zvariant, zbus_names and zvariant_derive, which zbus 6 holds: they go once AccessKit's adapter moves to zbus 6.
- **Style sheets and themes** ([ADR-0141](adr/0141-style-sheets-and-themes.md)).
  A field of the type `String` takes no named value.
  An error of type in a cooked style sheet or theme is placed by its byte and the path of its part, not by its line.
  The code cannot set a field of an element's style to its default against a rule.
  The styles made are kept up to a number of the elements' own styles for each set of rules (`Ui::set_style`).
  Past it, an element's style is made again through reflection at each change, the slow way that [`docs/measurements.md`](measurements.md), M6b.4b, measures.
  Selectors hold no combinators but an ancestor's kind or class.
  A color is held whole in a part of a style by `slime-ui`'s own walk, beside reflection's, which holds a rotation whole: two ways of one thing, which a type's attribute of reflection would unify.
  An array, a list and an option of an element's own style replace the rules' whole, its height with its width.
- **The system's fallback is checked only where the system has fonts**
  ([ADR-0136](adr/0136-the-interfaces-text.md)).
  Its test checks nothing, and says so, on a machine whose fontconfig, or whose lack of it, gives no font for Greek or Japanese, as CI's Linux runners may.
- **An editor's journal grows across recoveries** ([ADR-0135](adr/0135-the-editors-program.md)).
  Each recovery goes on with the journal that it recovered; only a start with nothing unwritten begins it anew.
  The history of steps outlives a stop only when something is unwritten.
  `editor_journal` fails once the journal is larger than the channel's largest message (`slime_serialize::MAX_SIZE`).
- **A selection made just before a crash is lost** ([ADR-0134](adr/0134-the-editors-core.md)).
  The journal writes a selection made apart from a step with the entry that follows it.
- **A journal whose last entry a crash of the system tore is not recovered**
  ([ADR-0135](adr/0135-the-editors-program.md)).
  The program stops; `--discard` sets the journal aside with every step that it holds, those before the tear included, which only a journal mended by hand plays again.
- **Two editors of one level, each with a journal of its own, do not see each other's changes**
  ([ADR-0135](adr/0135-the-editors-program.md)).
  The second to write the level is refused, the file changed apart from it, and its changes stay in its journal alone.

### Left by the audit of 0.4

Defects that the audit of 0.4 confirmed and that neither the audit pass nor the foundations pass
fixes ([ADR-0097](adr/0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)).
Each has its number, its rank on [ADR-0096](adr/0096-the-files-the-engine-reads.md)'s scale and the stage it waits for.
G01 to G56 are groups of defects; B1 to B4 performance (the build, the world, the renderer, loading), each finding numbered within its group, B4-F01 for instance; C1 to C7 architecture.
A test that confirms each is kept outside the repository, and comes in with its fix.
The audit did not cover NVIDIA and Intel GPUs, X11, a game outside the workspace, the delay from a key to the image shown, nor determinism between Linux and Windows.

- **A mesh that a passing error failed is not tried again** (G16, medium; M6c.7).
  Since M6a.9, a mesh of an asset cache's request fails alone ([ADR-0128](adr/0128-asset-requests.md)), and is not asked for again until the cooked data changes, a passing error such as GPU memory running out included.
  A scene's load still fails whole.
  Trying a passing error again is to decide with the editor's asset browser.
- **On Windows, a `.meta` whose name differs from its source's by case only takes a new ID at every
  cook** (G20, medium; M6c.7), and `slime-cook watch` cooks again without end.
  The scan matches the two files by the file system's rule, which ignores case on NTFS, then looks the ID up by the exact path.
  Not run on Windows by the audit: it waits in the [list of checks by hand](#checks-by-hand-waiting-for-the-owner).
- **Importing a glTF file takes time quadratic in its named meshes** (G23, medium; M6c.7): 38 to 57 s for 80 000 meshes, as each name is counted against the whole list.
- **The world's storage keeps 4 MB per component type and per million slots, and gives nothing back
  after destruction** (G28, medium; Stage 10a).
  Each type keeps an index of 4 bytes per slot up to the highest slot that held it.
  Neither the index, the nodes nor the slots shrink ([ADR-0082](adr/0082-the-worlds-storage-and-handles.md)).
- **Masked foliage loses its coverage in its smaller mip levels** (G30, medium; Stage 9a).
  The cooker averages alpha from one level to the next: the share of texels above the cutoff falls in the levels drawn far away, down to none, and trees fade in the distance.
  Preserving coverage per level for textures that a masked material reads is to do before large exteriors.
- **`World::write_scene` writes, without an error, a level that cannot be opened again** (G31, medium; M6c.6).
  It never counts the entities that the file gives once its placements resolve, which the reader refuses beyond `MAX_ENTITIES`, nor what they take, refused beyond `MAX_SCENE_SIZE`.
- **Between two fixed steps, the children of a turning parent cut the arc's chord while their
  siblings stay on it** (G38, low; Stage 7).
  A child moved by its parent is drawn in a straight line between its two positions.
  Interpolating through the hierarchy is to decide before skeletal animation turns many hierarchies.
- **A string on several lines in a scene file checked out on Windows keeps its CRLF line ends**
  (G40, low; M6c.6).
  `.gitattributes` keeps `*.toml`, `*.meta` and `*.tsv` at LF, not `*.scene`: the value and the cooked bytes depend on the checkout.
- **A negative integer, or one too large, for an unsigned field gives a wrong message** (G42, low; M6c.6): "not an integer" or "not text", rather than its range.
- **The writer of documents accepts what the reader refuses** (G43, low; M6c.6): a reserved key `$…` at the root, and a field given twice, the second lost.
- **A camera target farther than about 2 × 10¹⁹ m draws nothing, without a message** (G44, low; later): `view_from_eye` squares the direction in `f32`.
- **`World::write_scene` loops without end when more than 1 679 616 entities need IDs from one
  base** (G45, low; M6c.6): it draws four base-36 characters until one is free.
- **`Registry::register_dynamic` accepts version 0, which the derive refuses** (G48, low; Stage 8): the values are written, then never read back, with a wrong message.
- **Two cooks in one process share their temporary files** (G49, low; M6c.7, before the editor cooks).
  Named after the process, they separate processes, not threads: one cook fails at random, naming a file that exists.
- **An output directory inside the sources breaks the next cook** (G50, low; M6c.7): cooked glTF scenes take the `.scene` extension of scene files, which the next scan reads as sources.
- **After a glTF file is moved with its `.meta`, the cooker's cache keeps the old path** (G51, low; M6c.7): the manifest names it, and warnings blame tables that are right.
- **Two `.meta` with one ID, one of them without a source, make the cook stop without saying which
  is the orphan** (G52, low; M6c.7).
- **`Manifest::from_bytes` refuses a valid manifest whose names are short** (G53, low; M6c.7): it assumes 45 bytes per entry where the least is 37.
- **With `--watch`, after the cooked data is cleaned, a reload that the game refuses leaves it
  unable to save** (G56, low; M6c.7, with cooking from the editor).
  `SceneLibrary::reload` forgets the changed scenes before the game accepts them, and the files of the old manifest are gone.
