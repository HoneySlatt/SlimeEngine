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

| Milestone | Status |
|---|---|
| [M6c.0 — Before the stage](#m6c0--before-the-stage) | Done |
| [M6c.1a — The window and its operations](#m6c1a--the-window-and-its-operations) | Done |
| [M6c.1b — Dialogs, the cursor held and the GPU's identity](#m6c1b--dialogs-the-cursor-held-and-the-gpus-identity) | Done |
| [M6c.2a — The viewport and its navigation](#m6c2a--the-viewport-and-its-navigation) | Done |
| [M6c.2b — Picking and the selection](#m6c2b--picking-and-the-selection) | Done |
| [M6c.2c — A second viewport and Blender's way](#m6c2c--a-second-viewport-and-blenders-way) | Done |
| [M6c.3a — Move with handles](#m6c3a--move-with-handles) | Done |
| [M6c.3b — Create and arrange](#m6c3b--create-and-arrange) | Done |
| [M6c.3c — Place prefabs and see the history](#m6c3c--place-prefabs-and-see-the-history) | Done |
| [M6c.4a — Play the level](#m6c4a--play-the-level) | Done |
| [M6c.4b — Edit while playing](#m6c4b--edit-while-playing) | Done |
| [M6c.4c — Build the game again](#m6c4c--build-the-game-again) | Done |
| [M6c.4d — Measure Play](#m6c4d--measure-play) | Done |
| [Checkpoint A — The owner tries the editor](#checkpoint-a--the-owner-tries-the-editor) | Done on Linux; the Windows virtual machine to come |
| [M6c.5a — The hierarchy](#m6c5a--the-hierarchy) | Done |
| [M6c.5b — Rearrange the hierarchy](#m6c5b--rearrange-the-hierarchy) | Done |
| [M6c.5c — The eye and the lock](#m6c5c--the-eye-and-the-lock) | Done |
| [M6c.5d — Regions of tabs](#m6c5d--regions-of-tabs) | Done |
| [M6c.5e — Layouts kept, floating panels and closed panels](#m6c5e--layouts-kept-floating-panels-and-closed-panels) | Planned |
| [M6c.5f — The inspector](#m6c5f--the-inspector) | Planned |
| [M6c.6 — Levels of several files, and links](#m6c6--levels-of-several-files-and-links) | Planned |
| [M6c.7 — Build from a kit](#m6c7--build-from-a-kit) | Planned |
| [Checkpoint B — The exit rehearsal](#checkpoint-b--the-exit-rehearsal) | Planned |
| [M6c.8 — Panels dragged to dock](#m6c8--panels-dragged-to-dock) | Planned |
| [M6c.9 — Windows check, documentation, release 0.7](#m6c9--windows-check-documentation-release-07) | Planned |

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

In the milestones done, "Debt:" names items of [`debt.md`](debt.md) by their titles.

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
  - In [`debt.md`](debt.md): winit's drag and drop makes Windows' main thread a single-threaded COM
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
  and the layouts of M6c.5e serving meanwhile.
- **Crates.** `slime-ui`, `slime-editor`.
- **Delivers.** Zones to drop a panel or a tab on, shown as it is dragged; tabs reordered and moved
  by drags, and a button of their own that closes them; floating panels docked again by a drag; a
  least size for each panel. M6c.5e delivered the rest that this milestone once listed
  ([ADR-0174](adr/0174-layouts-kept-floating-panels-and-closed-panels.md)).
- **Done when.** `slime-ui`'s tests of the drops' rules; an agent drops a panel on a zone as a drag
  would; CI is green.
- **How to test.** `cargo test -p slime-ui -p slime-editor`.
- **By hand.** Panels docked by drag on the owner's Linux desktop and in the Windows virtual machine.
- **Debt settled.** Tabs without a button that closes them, nor dragged; two panels without a least
  size; a floating panel that docks by "Move to" alone.

### M6c.9 — Windows check, documentation, release 0.7

- **Goal.** Stage 6c is done, checked on Windows, and released as 0.7.
- **Delivers.** The owner's check, in the Windows virtual machine, of the programs that CI builds,
  with the checks that wait in [the list](#checks-by-hand-waiting-for-the-owner) if the owner makes
  them then; the documents reread as a newcomer would read them; `CHANGELOG.md`'s section of 0.7;
  the release, as [Releasing](../CONTRIBUTING.md#releasing) says; Stage 6c moved to [`history.md`](history.md);
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
  cooks it again without end (see [Left by the audit of 0.4](debt.md#left-by-the-audit-of-04)).

## Known debt

The known debt still open, 159 items, is in [`debt.md`](debt.md).
