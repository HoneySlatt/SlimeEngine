# Plan

This file says what is being worked on now. It lists the milestones of the current stage, their
status, how each one is checked, and the known debt. Direction beyond this stage is in
[`ROADMAP.md`](ROADMAP.md); decisions are in [`adr/`](adr/README.md). A *stage* is a step of the
roadmap; a *milestone* is a small, verifiable step of a stage.

Every milestone ends with the definition of done in [`AGENTS.md`](../AGENTS.md#definition-of-done):
build, tests, clippy, rustfmt and rustdoc pass, the program was actually run, and the affected
documentation was updated and reread. From Stage 2 on, every milestone is also checked on Windows
([ADR-0014](adr/0014-amend-roadmap-for-stage-2-and-windows.md)).

## Current

- Stage 6b — The engine's interface: done, and released as 0.6 on 2026-10-27
  ([ADR-0131](adr/0131-stage-6b-and-stage-6c.md) to
  [ADR-0154](adr/0154-floating-panels-and-held-keys.md)). The engine has a UI framework of its own,
  on which the dev overlay of *Hunna Wood* and of the examples runs, on Linux and on Windows: text
  in several writing systems, a tree of elements styled by themes read from files, input, focus and
  layers, text edited through input methods and the clipboard, widgets, menus, a palette of
  commands and notifications, read and used by agents through the MCP server and read by screen
  readers; egui has left the engine. The editor's core changes a level through commands, undone,
  redone and journaled, and writes it back with its comments, driven by an agent through the
  editor's own program, without a window yet. See [Completed stages](#completed-stages).
- Stage 6c — The editor: planned on 2026-10-27 ([ADR-0155](adr/0155-the-editors-design.md)), to be
  released as 0.7 at its close. A study of the editor's design, made by four agents from four
  starting points and measured in prototypes on the owner's machine, led to the owner's sixteen
  decisions on the editor's design, its milestones and their order, which ADR-0155 records. M6c.0
  is done, and M6c.1 was split in two at its start, M6c.1a and M6c.1b, both done; M6c.2 was split in
  two at its start too; next: M6c.2a.

## Status

| Milestone | Goal | Status |
|---|---|---|
| [M6c.0](#m6c0--before-the-stage) | The development channel answers in milliseconds, CI's checks stay within their limit, and a game that a tool started never outlives it | Done |
| [M6c.1a](#m6c1a--the-window-and-its-operations) | The editor opens a level in a window of its own, where a person and an agent run the same named operations | Done |
| [M6c.1b](#m6c1b--dialogs-the-cursor-held-and-the-gpus-identity) | A person opens a level through the system's dialog, a viewport can hold the cursor and read the mouse's raw motion, and a GPU object cannot act on another `Gpu`'s | Done |
| [M6c.2a](#m6c2a--the-viewport-and-its-navigation) | The level shows in a viewport that the user flies through, drawn again only when what it shows changed | Planned |
| [M6c.2b](#m6c2b--picking-and-the-selection) | The user picks in the viewport, the selection outlined, and a second viewport shows the level | Planned |
| [M6c.3](#m6c3--move-and-place) | The user moves, turns, scales, creates and deletes, each gesture a single step of the history | Planned |
| [M6c.4](#m6c4--play-it) | Play runs the level being edited in the game, steps not written included, and the editor's edits reach the running game | Planned |
| [Checkpoint A](#checkpoint-a--the-owner-tries-the-editor) | The owner tries the editor on Linux and in the Windows virtual machine, and the plan is adjusted | Planned |
| [M6c.5](#m6c5--find-and-change) | The user finds any entity in a hierarchy, changes its components in an inspector, and arranges the panels in regions of tabs | Planned |
| [M6c.6](#m6c6--levels-of-several-files-and-links) | A level is made of several files, which the editor writes together, and an entity refers to entities of other files | Planned |
| [M6c.7](#m6c7--build-from-a-kit) | The user builds a level from a kit of prefabs found in an asset browser, and *Hunna Wood*'s zone is made so | Planned |
| [Checkpoint B](#checkpoint-b--the-exit-rehearsal) | The owner builds a part of the zone again by hand in the editor, plays it, writes it and opens it again: the exit criterion | Planned |
| [M6c.8](#m6c8--panels-dragged-to-dock) | The user drags panels and tabs to arrange the editor; may move to Stage 11 | Planned |
| [M6c.9](#m6c9--windows-check-documentation-release-07) | Stage 6c is done, checked on Windows, and released as 0.7 | Planned |

Stages 0, 1, 2, 3a, 3b, 3c, 3d, 4a, 4b, 5a, 5b, 6a and 6b are done, and so are the audit pass and
the foundations pass; see [Completed stages](#completed-stages).

## Stage 6c — The editor

Exit criterion (see [`ROADMAP.md`](ROADMAP.md#stage-6c--the-editor--07)): a level of the game is
built, saved and reloaded entirely in the editor, with undo and redo on every operation; the editor
uses no private engine API. Released as 0.7 ([ADR-0131](adr/0131-stage-6b-and-stage-6c.md)).

This is the third part of Stage 6 ([ADR-0131](adr/0131-stage-6b-and-stage-6c.md)): the editor, a
client of the engine on the engine's interface, which a person and an agent use alike. Where the
engine stands at the start:

- **The editor has a core and a program, without a window**
  ([ADR-0134](adr/0134-the-editors-core.md), [ADR-0135](adr/0135-the-editors-program.md)): commands
  that name entities by their persistent identities, steps undone, redone and journaled, the
  selection, a level of one scene file written back with its comments; the `editor` example and
  *Hunna Wood*'s editor package, which an agent drives through the MCP server. A presentation can
  only queue the world's commands, each its own step: it can neither ask the core to undo, to
  select or to write, nor read the selection and the history.
- **The engine's interface** ([ADR-0132](adr/0132-the-engines-ui-framework.md), ADR-0140 to
  [ADR-0154](adr/0154-floating-panels-and-held-keys.md)) has widgets, layers, a menu bar, a palette
  of commands, notifications and floating panels, read by agents and by screen readers; the dev
  overlay's editors of reflected values and its world panel run on it
  ([ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)). It has no docking, no dialogs
  to open and save, no drag and drop between elements, no rows that are selected.
- **Views, picking and drawing for the editor**
  ([ADR-0126](adr/0126-views-apart-from-the-renderer.md) to [ADR-0130](adr/0130-picking.md)): a
  view draws into a target of its own, which the canvas shows; picking by the GPU and by the CPU;
  lines and unlit instances in the scene, lines over the image. The scene's depth is not resolved
  for an outline, and nothing is drawn over the image with a depth of its own.
- **The platform reads keys, the mouse, input methods and the clipboard of one window**, without
  the mouse's raw motion, a cursor held or files dropped (issue
  [#3](https://github.com/HoneySlatt/SlimeEngine/issues/3)), and **a frame runs at the scheduled
  rate** whether anything changed or not ([ADR-0021](adr/0021-frame-pacing.md)).
- **An MCP server drives one program at a time.** The development channel took most of a tenth of
  a second to answer a request, which Nagle's algorithm cost, until M6c.0
  ([ADR-0155](adr/0155-the-editors-design.md)).
- **Play does not exist, and a level is one scene file**, without parts, former identities or
  `EntityRef` ([ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md)).
- ***Hunna Wood*'s zone is laid out by its Blender script**, its trees, rocks and ferns scattered
  from a seed into one glTF file that a scene file places with six lanterns.

Out of scope: panels in windows of their own (one window per process); a highlight that follows
the pointer; the world updated in place keeping a game's handles (Stage 8); files dropped on
Wayland, and placed where they fall, which wait for winit 0.31, whose move is a milestone of its
own when it is released and does not hold 0.7 back; named layouts; a start page or a manager of
projects; modal keys G, R and S; a wireframe of the whole scene; a fix of the renderer for
[ADR-0041](adr/0041-render-graph.md)'s stall, unless the owner brings it in; editors of materials,
animation, effects and sound, and the rest of Stage 11's editor; a game's own editing tools (Stage
8's plugins, [ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md)); story layers (Stage 9b);
cells and streaming (Stage 10a).

Decisions taken when the stage was planned, by the owner on 2026-10-27: the editor's design, the
milestones, their order, the owner's two checkpoints, one release, and the dependencies, in
[ADR-0155](adr/0155-the-editors-design.md). Each milestone that designs a part records its design
in a new ADR, which amends ADR-0155 where it changes it; each dependency is added at the milestone
that first uses it, rfd at M6c.1b, `serde_json` in `slime-editor` at M6c.4, notify at M6c.7, what it
adds to CI's build checked then. Every milestone ends, besides the definition of done, with an
agent's session in `scripts/check.sh`, in a headless Weston when it is windowed, and image tests of
the panels that it adds. Any milestone may be cut in two at its start, as Stage 6b's were; M6c.1
and M6c.2 were, and M6c.5 and M6c.6 are expected to be.

Risks, whose figures ADR-0155's context gives:

- **[ADR-0041](adr/0041-render-graph.md)'s stall decides how the editor feels on machines without
  resizable BAR**, the owner's included: a view drawn again costs several milliseconds of CPU once
  the desktop fills the GPU's memory that the CPU maps. Drawing on demand hides most of it; M6c.2a
  measures `PASSES_PER_SUBMISSION` and the bloom there. The editor and the game running at once,
  two devices on one GPU, and Windows were never measured.
- **The cursor held and the mouse's raw motion were never run**, on any system: the study read
  winit's sources. M6c.1b tries them first, by hand, on niri, Hyprland, X11 through XWayland and in
  the Windows virtual machine, where relative motion may be missing.
- **No dialog was opened**: whether niri and Hyprland give `xdg-foreign` v2, without which a dialog
  is not modal to the window, and rfd off the main thread on Windows. M6c.1b's tests give the
  window the dialog's answers; the dialog itself is opened by hand. A path typed in the layer that
  waits for the dialog is the fallback
  ([ADR-0157](adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md)).
- **Play's figures are partial**: caches warm, nothing built, written or cooked, no level of 100 000
  entities. M6c.4 measures Play from the click; on Windows, a running executable may block the
  link. [ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md)'s second condition, that what the
  editor inspects through the channel is enough, cannot be tested before M6c.0's fix.
- **The open step and the order of the channel's requests**: what the journal holds after a crash
  during a drag, and how agents' steps that waited are applied. M6c.3 writes their tests first.
- **Writing several files whole or not at all**, beside `World::carry` and saves per scene
  ([ADR-0125](adr/0125-saves-per-scene.md)), and **opening leniently by splicing text** back: both
  unproven, M6c.6a starts with them.
- **The hierarchy's peaks**: in the study's prototype, a broad filter and a jump by a page passed a
  list's budget ([ADR-0140](adr/0140-the-tree-of-elements.md)). The filter runs off the frame or
  after a pause; M6c.5a measures with search and drags at 100 000 entities.
- **Costs never measured**: the outline and the handles at 3840 × 2160 with many entities selected;
  thumbnails at the scale of thousands of meshes, and drawn many at once; docking's drags and a
  docked layout's interface.
- **The size of the stage.** The study counted 31 000 to 39 000 lines of Rust with their tests,
  more than Stage 6b's framework. [Checkpoint A](#checkpoint-a--the-owner-tries-the-editor) is where
  to cut: docking by drag, stamp mode and modal keys are already outside what the exit needs.
- **Dates outside the project**: winit 0.31, in beta for months, and `accesskit_winit`'s release
  for it. Polling's cost on Windows is not measured, which M6c.7 measures before it adopts notify.
- **The cost of the checks.** M6c.0 brought each of CI's jobs well within the owner's limit of 20
  minutes, but every session of the editor adds to the checks ([Known debt](#known-debt)): each
  milestone records how long CI's jobs take.
- **One GPU.** Budgets are checked on the owner's RX 6900 XT only ([Known debt](#known-debt)).

### M6c.0 — Before the stage

- **Goal.** The development channel answers in milliseconds, CI's checks stay within their limit,
  and a game that a tool started never outlives it.
- **Crates.** `slime-channel`, `slime-app`; `scripts/check.sh` and CI's workflow; `AGENTS.md` and
  `CONTRIBUTING.md`.
- **Delivers.** As [ADR-0155](adr/0155-the-editors-design.md) decides: each message of the
  development channel written in one write, Nagle's algorithm turned off at both ends
  (`TCP_NODELAY`); the round trip measured again, in [`docs/measurements.md`](measurements.md), and
  what remains of the delay, if delayed acknowledgements still hold it, said. A game started with a
  token ends when its standard input closes, so that an editor, an MCP server or a test that crashes
  leaves no game behind, with the standard library alone. `scripts/check.sh` in three parts, which
  CI runs in three jobs side by side, which runs each part holds chosen at the milestone, the limit
  of 20 minutes kept ([ADR-0123](adr/0123-the-linux-checks-in-two-jobs.md)). `CONTRIBUTING.md`
  says that a windowed run measured on a GPU in a headless Weston needs `--renderer=gl`, without
  which RADV cannot present and wgpu falls back on lavapipe.
- **Done when.** A test of the channel's round trip, and its figure; a test in which a game started
  by a tool ends once its standard input closes; the commands of `AGENTS.md` and `CONTRIBUTING.md`
  name the three parts; CI is green, each job within its limit, their times recorded here.
- **How to test.** `cargo test -p slime-channel -p slime-app`; `scripts/check.sh` with each part.
- **Debt settled.** CI's Linux jobs past their limit of 20 minutes.
- **Outcome.** Done on 2026-10-27. Each message of the development channel goes out in one write,
  but for a payload, which follows in a second, Nagle's algorithm off at both ends: a request
  answered at once took 82 ms and takes 0.14 ms, and no delayed acknowledgement remains in it
  ([measurements](measurements.md#m6c0-ryzen-7-5800x)); `slime-channel`'s test measures it. A game
  started with a token ends once its standard input closes, which `slime_channel::launch` holds as
  a pipe, the run stopping as at a termination signal; a terminal there is left alone, and an input
  that is empty or missing, as `/dev/null`, ends the game at once, which the token's rustdoc says;
  `slime-app`'s `tests/channel.rs` checks it, and fails without it. CI's failures since 0.6 came
  from the game's agent session: each of its requests waits for a frame of the zone, which took
  about 25 s on CI's runners with its shadows, and it reached its own limit of 15 minutes. The
  session now sets the game's view without shadows, without anti-aliasing and at half its size once
  the game is paused, which the game's own runs in the checks do not: 426 s became 21 s on four
  cores, and 22 s on CI. `scripts/check.sh` has three parts: `headless`, every check that needs
  neither a compositor nor the zone; `windowed`, the examples and the windowed lifecycle test; and
  `game`, everything that reads the zone, which it alone fetches and cooks among the Linux jobs.
  `CONTRIBUTING.md` says how to measure a windowed run on a GPU in a headless Weston, which was
  checked: with Weston's default renderer, the `window` example took llvmpipe; with
  `--renderer=gl`, the RX 6900 XT. The review found the empty input's case undocumented, a read
  interrupted by a signal taken for the tool's end, and several documents that said less or other
  than the code, each fixed. CI is green on `a199369`: `check (headless)` took 13 minutes 35
  seconds, `check (windowed)` 4 minutes 42 seconds, `check (game)` 4 minutes 14 seconds with the
  cooked zone restored from its cache, `windows` 19 minutes 43 seconds and `windows-release` 13
  minutes 32 seconds. `windows` then ran some of its tests a second time to print what they measured
  on WARP, built again for it with other features; its one run of the tests now leaves their output
  uncaptured instead. CI is green on `bb4552b`: `check (headless)` 13 minutes 43 seconds, `check
  (windowed)` 5 minutes 31 seconds, `check (game)` 4 minutes 14 seconds, `windows` 11 minutes 20
  seconds and `windows-release` 10 minutes 10 seconds, on runners faster than the run before.

### M6c.1 — The editor's window

The owner split it in two at its start, on 2026-10-28: the window and its operations, then the
dialogs, the cursor held and the GPU's identity.

#### M6c.1a — The window and its operations

- **Goal.** The editor opens a level in a window of its own, where a person and an agent run the
  same named operations.
- **Crates.** `slime-editor`, `slime-app`, `slime-channel`, `slime-mcp`, `slime-ui`,
  `slime-platform`, `slime-devui`; the `editor` example and `games/hunnawood-editor`.
- **Delivers.** As [ADR-0155](adr/0155-the-editors-design.md) decides: the editor's window, a
  presentation of the editor's program that reads the core through a value they share and asks it
  through typed requests, the channel's `Edit` requests going through the same handler; the
  registry of operations, run by menus, shortcuts, the palette and agents alike, the channel's
  requests that list them and run one by name, version 10 of the protocol, and the MCP server's
  two tools; frames on demand in `slime-app`, and the interface drawn only when it changed; fixed
  regions of panels, the menu bar, a status bar, and the window's title, both showing how many
  steps are not written; quitting at once, the journal keeping what was not written; opening
  another level in the same window by a path given, which M6c.1b's dialog fills in for a person,
  saving under another name waiting for M6c.6, where a copy of a level gets its asset ID and its
  identities; the center of the window empty until M6c.2's viewport, with a word; the palette
  scrolling past 12 entries; roots named in paths by their keys, not by their numbers; the
  window's title changed while it runs; `slime-app`'s state and cache directories beside its data
  directory; the dev overlay's world panel in a region, as the hierarchy and the inspector until
  M6c.5; on the first run, the program's level opened at once, the examples cooked if missing, the
  recent levels in the File menu and one hint; the run without a window kept for CI's agents, which
  runs a frame only when a request comes too, as the owner decided on 2026-10-28. The ADR of the
  design.
- **Done when.** Tests without a GPU: the core answers the window's requests and the channel's
  alike; an operation run by its name, by its shortcut and from the palette does the same; the
  operations that change the view enter no history. A check finds every item of a menu and every
  entry of a `.keys` file in the registry. An agent's session in a headless Weston opens the yard,
  runs operations by name, undoes, and writes the level; an idle editor runs no frame for 10 s;
  image tests of the window at scales 1 and 2; CI is green.
- **How to test.** `cargo test -p slime-editor -p slime-app -p slime-ui -p slime-channel`;
  `cargo run --example editor -- --level <a copy of the yard> --journal <file>`; the MCP server's
  session tests.
- **Debt settled.** The title changed (issue #3, in part); a root's number in paths; the palette's
  12 entries.
- **Outcome.** Done on 2026-10-28, as
  [ADR-0156](adr/0156-the-editors-window-and-its-operations.md) records. The editor's program opens
  its level in a window in the editor's look, a menu bar, *File*, *Edit* and *Help*, the dev
  overlay's world panel in a region on the left, a center that holds no viewport yet, and a status
  bar, the title and the status bar saying how many steps the level's file does not hold
  (`Editor::unwritten`); `--headless` runs it without a window. The core and the window share a
  value, and the window asks the core through requests that the one handler of the development
  channel's requests answers too. Every action is an operation of one registry
  (`slime_editor::OPERATIONS`), run by name from the menus, the table of shortcuts
  (`editor.keys`), the palette and agents, through version 10 of the channel's protocol and the MCP
  server's `list_operations` and `run_operation`. `slime-app` runs a program's frames on demand
  (`AppConfig::on_demand`, which replaces the paced run without a window), woken by events, the
  channel's listener, the interface's needs and deadlines (`Ui::needs_frame`, `Ui::next_change`),
  `Context::request_frame` and a `Waker`; `slime-app` gives the user's state directory;
  `slime-ui` names a root by a key in agents' paths and scrolls the palette past 12 commands. The
  owner decided on 2026-10-28 that the center stays empty until M6c.2, that a level is opened from
  the window by a path with the recent levels in the File menu while saving under another name waits
  for M6c.6, and that the run without a window runs on demand too. Measuring the idle editor found
  it running frame after frame while a notification waited for its end, 281 frames in ten seconds
  on lavapipe; that end now wakes it once, at its deadline. The review found sixteen faults, each fixed: a test that wrote journals into the
  user's data directory and sessions that wrote the user's recent levels, now kept in directories
  of their own; on demand, a list's rows left unbuilt, a submenu opened by the pointer's rest left
  unshown, and the interface's clock stale after a wait; the world panel left selected when the
  core selected nothing; a level opened again without the journal that `--journal` names; the
  palette scrolled too far by keys between two frames; and documents that said other than the code.
  The tests: the core's requests from the window and the channel alike, an operation by its name,
  its shortcut, the palette and its menu item, the operations of the selection and the window out
  of the history, the menus' and `editor.keys`' names in the registry (`slime-editor`'s window
  tests); the steps not written; image tests of the window at scales 1 and 2
  (`examples/tests/editor.rs`); and an agent's session in a headless Weston that runs operations
  by name, opens the File menu and the palette, sees the editor wake for the end of the first
  run's hint, run no frame for ten seconds alone, and quit (`crates/slime-mcp/tests/edit_window.rs`).
  What nothing checks yet: the window on Windows, which the owner's check in the virtual machine
  at Checkpoint A covers. CI is green on `a20c15c`: `check (headless)` took 14 minutes 56 seconds,
  `check (windowed)` 6 minutes 2 seconds, `check (game)` 5 minutes 15 seconds, `windows` 15 minutes
  16 seconds and `windows-release` 11 minutes 23 seconds.

#### M6c.1b — Dialogs, the cursor held and the GPU's identity

- **Goal.** A person opens a level through the system's dialog, a viewport can hold the cursor
  and read the mouse's raw motion, and a GPU object cannot act on another `Gpu`'s.
- **Crates.** `slime-editor`, `slime-ui`, `slime-platform`, `slime-app`, `slime-gfx`;
  `flake.nix`.
- **Delivers.** As [ADR-0155](adr/0155-the-editors-design.md) decides: a dialog to open a level
  through rfd, on a thread of its own, which fills in for a person the path that an agent gives,
  and a scrim behind a modal layer; D-Bus's library in the development shell, which rfd loads; the
  mouse's raw motion and the cursor held, locked on Wayland and Windows, confined, hidden and put
  back on X11, left free with a word where locking fails; each `Gpu` with an identity that its
  objects carry, an object used with another `Gpu` panicking (G34). The dependency rfd 0.17, which
  [ADR-0132](adr/0132-the-engines-ui-framework.md) approved.
- **Done when.** Tests: the operation that opens a level takes a path without a dialog, and the
  dialog's answer runs the same operation; a modal layer's scrim in an image test; the raw motion
  read from the platform's events; an object of one `Gpu` used with another panics. CI is green.
- **How to test.** `cargo test -p slime-editor -p slime-ui -p slime-platform -p slime-gfx`;
  `cargo run --example editor -- --level <a copy of the yard> --journal <file>`.
- **By hand.** The dialog opened on niri, Hyprland and in the Windows virtual machine; the
  cursor held and the raw motion on niri, Hyprland, X11 through XWayland and in the Windows virtual
  machine.
- **Debt settled.** The raw motion and the cursor held (issue #3, in part); a modal layer without a
  scrim; G34.
- **Outcome.** Done on 2026-10-28, as
  [ADR-0157](adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md) records. The editor's
  *Open a level…*, `file.choose` (Ctrl+O, the File menu's first item, the palette), opens a modal
  layer over the window, which its backdrop dims, with a field for the level's path, and the
  window opens the system's dialog meanwhile, through rfd on a thread of its own
  (`slime_platform::Window::choose_file`, `Event::FileChosen`); the file chosen runs `file.open`,
  the operation that an agent runs with a path. A dialog that answers no file sooner than a person
  could, where no portal and no `zenity` answer, leaves the layer waiting for the path typed; a
  program that a tool started opens no dialog of the system, and its layer waits for the path.
  `slime-ui` gains `PaintStyle::backdrop`, which a layer's root lays over the window below it, and
  gives a modal layer's root the class `modal`, which the editor's look dims with its theme's
  scrim. `slime-platform` holds the cursor, hidden (`Window::set_cursor_held`), locked on Wayland
  and Windows, confined and put back on X11, lets it go as the window loses the focus, and reports
  the mouse's raw motion while it holds it (`Event::RawMotion`); the `window` example holds it
  while its right button is held. Each `Gpu` has an identity that its objects carry, a readback
  included, and a use of an object with another device panics at the caller's line (G34). The
  review found eleven points, each settled: an agent could still reach the system's dialog through
  the menu, the palette or Ctrl+O, the channel alone refusing it, which a program that a tool
  started now settles by opening no dialog; on Windows, winit 0.30 gives a tablet's positions as
  raw motion, which the rustdoc, the ADR and the debt state; the panic of a mixed object named
  `slime-gfx`'s line, not the caller's; a readback was not checked; the panics were missing from
  the methods' documentation; a panic of rfd on its thread left the layer waiting; a lock held
  across winit's calls could deadlock on Windows; a level named without a directory gave the
  dialog no directory to start in; and documents that copied facts or said less than the code. The
  tests: the dialog's answers given by number, a file, none at once, none later, Escape, Cancel and
  a late answer not heard, and the path typed, in a window that opens dialogs and in one that a
  tool started (`slime-editor`'s window tests); the backdrop of a modal layer in the look, and none
  for another (`crates/slime-ui/tests/look.rs`); the window waiting for the dialog in an image test
  (`examples/tests/editor.rs`, `editor-choose.png`); an agent's session that opens another copy of
  the yard through *Open a level…*, its path typed (`crates/slime-mcp/tests/edit_window.rs`); the
  raw motion read from the platform's events; an object of one `Gpu` used with another, and one of
  a lost device used with its replacement, panicking (`crates/slime-gfx/tests`). What nothing
  checks yet: the dialog on a desktop, the cursor held and the raw motion, which the owner checks by
  hand on niri, Hyprland, X11 through XWayland and in the Windows virtual machine. A path mistyped
  in the layer closes it before `file.open` refuses it, which a notification says. CI's first run
  found the screen readers' check broken on its Ubuntu runner, where AT-SPI's bus launcher, its path
  putting `/usr/bin` first, started the system's D-Bus daemon with the development shell's D-Bus
  library, which it cannot load: its services now run without the shell's library path (`49620a2`).
  CI is green on `49620a2`: `check (headless)` took 14 minutes 3 seconds, `check (windowed)` 5
  minutes 56 seconds, `check (game)` 6 minutes 41 seconds, `windows` 14 minutes 40 seconds and
  `windows-release` 13 minutes 32 seconds.

### M6c.2 — The viewport

Split in two at its start, on 2026-10-28, as [ADR-0155](adr/0155-the-editors-design.md) allows:
the viewport and its navigation, then picking and the selection.

#### M6c.2a — The viewport and its navigation

- **Goal.** The level shows in a viewport that the user flies through, drawn again only when what
  it shows changed.
- **Crates.** `slime-ui`, `slime-render`, `slime-editor`; the benchmarks.
- **Delivers.** As [ADR-0155](adr/0155-the-editors-design.md) decides: `Kind::Surface`, an element
  kind that gives the program the pointer's actions and holds the pointer during a drag, single-key
  shortcuts suspended while the right button flies; the level drawn in the window's center by the
  editor's own asset cache and frame builder, through a view whose image the interface shows;
  navigation, flying with the right button held and W, A, S, D, Q, E, the cursor held and the raw
  motion read, orbiting with Alt and the left button, panning with the middle one, F framing the
  selection, and a `.keys` preset in Blender's way; the grid; icons of lights and cameras from
  Lucide's glyphs; the viewport drawn again only when its camera, the world, the selection, its
  settings or its assets changed, and once more `RESIZE_DELAY` after its size last changed; the
  viewport named for agents, who capture it and change its settings; `PASSES_PER_SUBMISSION` and
  the bloom measured while the desktop fills the GPU's memory that the CPU maps. The ADR of the
  design.
- **Done when.** Image tests on lavapipe of the grid and the icons; an agent captures the viewport
  and flies it with scripted keys; an idle editor's viewport draws nothing; the frame's costs at
  the hardware tiers' sizes, in [`docs/measurements.md`](measurements.md); CI is green.
- **How to test.** `cargo test -p slime-ui -p slime-render -p slime-editor`;
  `cargo run -p hunnawood-editor -- --level <a copy of the zone> --journal <file>`.
- **By hand.** Flying and orbiting in the zone on niri, Hyprland and X11 through XWayland.
- **Debt settled.** A view's new size waiting for its next frame.

#### M6c.2b — Picking and the selection

- **Goal.** The user picks in the viewport, the selection outlined, and a second viewport shows the
  level.
- **Crates.** `slime-render`, `slime-world-render`, `slime-editor`.
- **Delivers.** As [ADR-0155](adr/0155-the-editors-design.md) decides: a click that picks by the
  GPU, the outermost placement first and a level deeper at the second click, Alt and a click
  through what lies behind by the CPU's ray, a marquee selecting what its area holds; the selection
  outlined, the scene's depth resolved, its hidden part dimmed, an entity without a mesh drawn as
  its transform's unit box; the modes of debugging base color and light alone, and a wireframe of
  the selection; a second viewport, the one navigated alone drawn at the full rate. The ADR of the
  design.
- **Done when.** Image tests on lavapipe of the outline and its hidden part; an agent picks in a
  viewport; CI is green.
- **How to test.** `cargo test -p slime-render -p slime-world-render -p slime-editor`;
  `cargo run -p hunnawood-editor -- --level <a copy of the zone> --journal <file>`.
- **By hand.** Picking in the zone on niri, Hyprland and X11 through XWayland.
- **Debt settled.** The scene's depth resolved
  ([ADR-0129](adr/0129-drawing-for-the-editor.md)); the outline; the two modes of debugging.

### M6c.3 — Move and place

- **Goal.** The user moves, turns, scales, creates, duplicates and deletes what the level holds,
  each gesture a single step of the history.
- **Crates.** `slime-editor`, `slime-render`, `slime-world`; the `editor` example.
- **Delivers.** As [ADR-0155](adr/0155-the-editors-design.md) decides: the core's open step, a
  gesture applied live, cancelled by Escape, made one step as it ends, the requests of agents
  waiting meanwhile; handles to move, turn and scale, the engine's own, drawn over the image with a
  depth of their own, in the world's axes or the selection's, about a pivot, onto the world's
  uniform scale and its stretch; snapping by increments of distance, angle and scale, and to an
  absolute grid; End and Shift+End dropping the selection on what lies below, by the CPU's picker;
  creating, duplicating (Ctrl+D, the references inside the copy pointed at the copy, nested
  overrides kept), deleting, copying and pasting as commands' text on the system's clipboard,
  repeating the last operation, grouping (Ctrl+G) and nudging; "Place prefab…" from the
  manifest's scenes; the history panel. The ADR of the design.
- **Done when.** Tests of the core written first: an open step applied live, cancelled, and made one
  journaled step; an agent's step that waited applied after it; a crash during a drag losing the
  drag alone. Tests of the handles' mathematics; image tests of the handles; an agent moves three
  entities in one step and undoes it; the zone written back unchanged once every step is undone; CI
  is green.
- **How to test.** `cargo test -p slime-editor -p slime-render`; the MCP server's session tests.
- **By hand.** The handles' feel, their hit areas and the snapping, in the yard and the zone.
- **Debt settled.** Instances over the image, with a depth of their own
  ([ADR-0129](adr/0129-drawing-for-the-editor.md)).

### M6c.4 — Play it

- **Goal.** Play runs the level being edited in the game, steps not written included, and the
  editor's edits reach the running game.
- **Crates.** `slime-editor`, `slime-app`, `slime-world`, `slime-channel`, `slime-mcp`;
  `games/hunnawood` and `games/hunnawood-editor`.
- **Delivers.** As [ADR-0155](adr/0155-the-editors-design.md) decides: Play's states and its
  controls, Play, Pause, Step, Speed and Stop, in a toolbar; the game started through
  `slime_channel::launch`, with a token and a save of the editor's own, given the scenes that hold
  steps not written as private copies, through an option of development builds that the engine
  handles for every game, the user's files left alone; the steps made while it runs sent to it as
  commands, those it refuses shown; a console of its log; a game whose code changed built again and
  started on a save of its run, a mark on Play saying so, the build read from cargo's messages in
  JSON and the executable started itself, without a console window on Windows; the MCP server
  driving two programs, the editor and the game it plays; Play's time to control measured from the
  click on the zone and on a generated level of 100 000 entities, against ADR-0110's 3 seconds;
  "keep this value", a value read in the running game made a step of the level, if the milestone has
  room. The dependency `serde_json` in `slime-editor`, already in the lock. The ADR of the design,
  which says how the scenes are handed over.
- **Done when.** Play's states tested against a stub game; an agent plays, pauses, steps, reads the
  log and stops; a game that crashes leaves the editor running; an edit while playing reaches the
  game, and one that the game refuses is shown; Play leaves the user's files as they were; the
  figures in [`docs/measurements.md`](measurements.md); CI is green.
- **How to test.** `cargo test -p slime-editor -p slime-app`;
  `cargo run -p hunnawood-editor -- --level <a copy of the zone> --journal <file>`.
- **By hand.** Play, edits while playing, and a change of the game's rules built and started again,
  on Linux.
- **Debt settled.** An entity that a run created, wrote into its level and destroyed coming back
  ([ADR-0124](adr/0124-driven-components-keys-and-created-entities-in-levels.md)); an MCP server
  that drives one program at a time.

### Checkpoint A — The owner tries the editor

- **What.** The owner, for about an hour, on Linux and in the Windows virtual machine: flies
  through the zone, selects, rearranges it and plays it; the raw motion, the cursor held, the
  game's child process and the system's firewall, the dialogs.
- **Result.** A list of frictions, and this plan adjusted: what to cut, if the stage runs long, is
  decided here.

### M6c.5 — Find and change

- **Goal.** The user finds any entity in a hierarchy, changes its components in an inspector, and
  arranges the panels in regions of tabs.
- **Crates.** `slime-ui`, `slime-world`, `slime-devui`, `slime-editor`; the benchmarks.
- **Delivers.** As [ADR-0155](adr/0155-the-editors-design.md) decides, in two parts, each likely a
  milestone of its own:
  - *M6c.5a, the hierarchy and the regions.* In `slime-ui`: the roles of a tree and its items, rows
    that are selected, drag and drop between elements, an element scrolled into view, panes that
    F6 moves between, an amendment of [ADR-0153](adr/0153-tab-and-f6-in-a-games-interface.md);
    docking's model, regions of tabs, splits that the user resizes, "Move to" and floating panels,
    the layout saved per user and per project in the user's state directory, and set aside when it
    cannot be read. In `slime-world`, a tick of the entities' names apart from the structure. The
    hierarchy: rows that the editor flattens from the world's children, built only where they
    show; a search, `c:` naming a component and `p:` a prefab, run off the frame; renaming;
    reparenting by a drag and by Alt and the arrows; an eye and a lock, the user's, never the
    level's, the editor hiding nothing through `World::is_hidden`.
  - *M6c.5b, the inspector.* `slime-devui`'s builder of an entity's components made public; the
    inspector on it: components added and removed; several entities selected, a mixed value shown
    as "—", a check box's third state; overrides marked by a bar and a bold label, Revert and Apply
    for each field; driven fields read-only; the interim world panel retired.
  The ADR of each part's design.
- **Done when.** A benchmark, `editor.hierarchy.100k`, within a list's budget
  ([ADR-0140](adr/0140-the-tree-of-elements.md)) standing still, scrolled and after a rename; an
  agent reparents by a drag's action and reverts an override; a layout that cannot be read gives the
  default; image tests of the hierarchy and the inspector; CI is green.
- **How to test.** `cargo test -p slime-ui -p slime-world -p slime-devui -p slime-editor`;
  `cargo run --release -p slime-benches -- editor`.
- **By hand.** The hierarchy and the inspector, read by Orca on Linux and by Narrator in the
  Windows virtual machine.
- **Debt settled.** G32; a list's rows not selected; the focus moved not scrolling its element
  into view, and nothing scrolling an element into view for screen readers; a check box without a
  third state; a row of a list found by its number; the world panel's list made again at each
  rename.

### M6c.6 — Levels of several files, and links

- **Goal.** A level is made of several files, which the editor writes together, and an entity
  refers to entities of other files.
- **Crates.** `slime-world`, `slime-serialize`, `slime-reflect`, `slime-cook`, `slime-devui`,
  `slime-editor`; `.gitattributes`; the examples' yard and *Hunna Wood*'s zone.
- **Delivers.** As [ADR-0155](adr/0155-the-editors-design.md) and
  [ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md) decide, in two parts, each likely
  a milestone of its own:
  - *M6c.6a, levels of several files.* The new layout of scene files, in one change, which the
    first part holds whole: the level's parts in a table of their own, former identities, the
    absolute identities that `EntityRef` writes, and whether a file says the parents and the order
    of a placement's entities. The core over several files: a step across files, the journal's
    entries naming their files, every file written or those named, all or none; a level that holds
    a component or a field that the program does not know opened, its text kept and written back
    byte for byte; a check before writing that the placements of a changed prefab still spawn;
    prefabs changed on disk followed; a prefab opened in context. The yard and the zone written in
    the new layout.
  - *M6c.6b, links.* `EntityRef`, and its picker in the inspector: an eyedropper in the viewport, a
    drag from the hierarchy, and a search over the parts loaded or not; a prefab made of a
    selection; Apply to prefab.
  The ADR of each part's design, the first amending
  [ADR-0091](adr/0091-scene-documents-and-placements.md) for the layout.
- **Done when.** `slime-world`'s round trips on fixtures of the new layout; tests of the core's
  steps across files; a failure injected between two renames leaves the files as they were, or
  names those written; one edit changes one hunk of a file; a level with an unknown component
  opened, changed and written back with that component's text unchanged; an agent splits the yard
  into parts and links an entity across two; CI is green.
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
- **Delivers.** As [ADR-0155](adr/0155-the-editors-design.md) decides: the asset browser, an index
  of the cooker's manifest and a scan of the sources, by folder and kind, with a search;
  thumbnails, a texture's small level of its KTX2, a mesh or a prefab drawn offscreen, a few a
  frame and only for the tiles that show, encoded as PNG with `Compression::Fast` and cached in the
  user's cache directory under the cooked file's name; assets dragged into a viewport; stamp mode,
  which may be cut; New level; cooking in the editor's process, on the background priority, with its
  progress, once two cooks in one process no longer share their temporary files (G49); the tools'
  watch told of changes by notify, once polling is measured on Windows, a scan again when its queue
  overflows and polling as its fallback; files dropped on Windows and X11 imported; the interface's
  images asked of the asset cache. *Hunna Wood*'s kit: its Blender script exports pieces of terrain
  and one prefab per variant of tree, rock and prop with its collider; an agent converts the
  scatter into placements once, through the editor's operations; the zone's tests on the kit, and a
  new save fixture, which [ADR-0100](adr/0100-updates-of-shipped-games.md) allows before the game's
  first release; the game's README says it. The dependency notify 8.2.1. The ADR of the design. The
  known debt gains that winit's drag and drop makes Windows' main thread a single-threaded COM
  apartment, which Stage 7's audio must not start as a multi-threaded one.
- **Done when.** Thumbnails found in the cache after a restart; tests of the watch: a file written,
  renamed and removed, and a queue that overflowed; an agent places an asset by its ID and cooks;
  the zone's tests pass on the kit, with the new fixture; CI is green.
- **How to test.** `cargo test -p slime-editor -p slime-cook -p slime-assets`;
  `scripts/fetch-assets.sh`;
  `blender --background --factory-startup --python games/hunnawood/zone/build_zone.py`;
  `cargo test -p hunnawood -p hunnawood-editor -- --ignored`.
- **By hand.** The browser, its thumbnails and stamping, on Linux; G20, which waits in
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
  milestone moves to Stage 11, with an amendment of ADR-0131 and the roadmap, the regions of M6c.5a
  serving meanwhile.
- **Crates.** `slime-ui`, `slime-editor`; `games/hunnawood-editor`.
- **Delivers.** As [ADR-0155](adr/0155-the-editors-design.md) decides: zones to drop a panel or a
  tab on, shown as it is dragged; tabs moved and closed; floating panels docked again; closed
  panels coming back to their places; a least size for panels; a project's default layout, which a
  game's editor package gives. The ADR of the design.
- **Done when.** `slime-ui`'s tests of the layout's rules after random edits; a layout saved against
  an older default upgraded; a layout that cannot be read gives the default and a notification; an
  agent moves a panel by its operation; CI is green.
- **How to test.** `cargo test -p slime-ui -p slime-editor`.
- **By hand.** Panels docked by drag on niri, Hyprland and in the Windows virtual machine.
- **Debt settled.** Tabs neither moved nor closed; two panels without a least size; a floating
  panel that does not dock.

### M6c.9 — Windows check, documentation, release 0.7

- **Goal.** Stage 6c is done, checked on Windows, and released as 0.7.
- **Delivers.** The owner's check, in the Windows virtual machine, of the programs that CI builds,
  with the checks by hand that wait in [the list](#checks-by-hand-waiting-for-the-owner) if the
  owner makes them then; the documents reread as a newcomer would read them; `CHANGELOG.md`'s
  section of 0.7; the release, as [Releasing](#releasing) says; Stage 7 planned with the owner.
- **Done when.** The owner ran *Hunna Wood*'s editor on Windows; the documents are reread and
  corrected; CI is green on the release commit; the tag and the GitHub release exist, with the
  Windows programs.
- **By hand.** In the Windows virtual machine: *Hunna Wood*'s editor, its dialogs, its viewport,
  Play, and the asset browser.

### What the exit needs at minimum

If the stage must be cut, the exit needs M6c.1 to M6c.3, M6c.5 and M6c.6, with prefabs placed from
a list. It can do without panels docked by drag (M6c.8), thumbnails, a list of names serving, stamp
mode, several viewports, a view of the running game and "keep this value", the notify watch, and
modal keys. Play ([ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md),
[ADR-0131](adr/0131-stage-6b-and-stage-6c.md)) and the layout of files
([ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md)) belong to the stage by decision,
not by the exit. What is cut is decided at
[Checkpoint A](#checkpoint-a--the-owner-tries-the-editor).

## Checks by hand waiting for the owner

The checks by hand of the milestones done that the owner has not made yet. The owner decided on
2026-10-20 to make them later, together: each milestone adds its own here when it is done, and a
check leaves the list once made, its result written in its milestone's outcome while the milestone
is in this plan, and in its stage's row of [Completed stages](#completed-stages) once it has left.
The game is run with `--new --save <a file of its own>`, so that the owner's save is left alone.

- **G20, in the Windows virtual machine.** A source asset whose `.meta` differs from its own name
  by case only, cooked twice: whether its ID changes at each cook, and whether `slime-cook watch`
  cooks it again without end (see [Known debt](#left-by-the-audit-of-04)).

## Releasing

A release is an annotated git tag on a commit whose CI run is green on Linux and Windows
([ADR-0004](adr/0004-workspace-layout-and-crate-naming.md)):

1. In one commit, set the version in the root `Cargo.toml` and the tag that the README's "Using
   SlimeEngine in a game" names, with its list of crates, rename the `[Unreleased]` section of `CHANGELOG.md` to
   `[<version>] - <date>`, and update this plan.
2. Push that commit and wait for its CI run to pass.
3. Tag that commit, not a later one, then push the tag:
   `git tag -a v<version> -m "SlimeEngine <version>" <commit>`.
4. From 0.1 on, create the GitHub release of the tag, its `CHANGELOG.md` section as its notes,
   with the `slime-examples-windows` artifact of that commit's CI run attached as
   `slime-examples-windows-<version>.zip`: CI keeps its artifacts 14 days, a release keeps them.
   CI puts beside the programs DXC's library and its licence, the licenses of the parts of Noto
   fonts that `interface.exe` embeds and those of Geist, Geist Mono and Lucide, which `widgets.exe`
   embeds, and `scene.exe`, `world.exe` and `hunnawood.exe` in their dev overlay; add
   `LICENSE.txt` and a `NOTICE.txt`.

0.0.1 was released on 2026-10-12, before Windows joined CI: the tag `v0.0.1` is on the commit
"Release 0.0.1". 0.0.2 was released the same way on 2026-10-12: the tag `v0.0.2` is on the
commit "Release 0.0.2", and 0.0.3 on 2026-10-13: the tag `v0.0.3` is on the commit
"Release 0.0.3". 0.1 was released on 2026-10-14: the tag `v0.1.0` is on the commit "Release 0.1.0",
and its GitHub release carries the Windows examples that CI built from that commit.
0.2 was released on 2026-10-15: the tag `v0.2.0` is on the commit "Release 0.2.0", and its GitHub
release carries the Windows programs that CI built from that commit, *Hunna Wood* with its zone
cooked and the notice that credits its assets among them. 0.3 was released on 2026-10-17, with
Stages 4b and 5a: the tag `v0.3.0` is on the commit "Release 0.3.0", and its GitHub release carries
the Windows programs that CI built from that commit, `world.exe` among them.
Stage 5b was released as 0.4 on 2026-10-18: the tag `v0.4.0` is on the commit "Release 0.4.0", and
its GitHub release carries the Windows programs that CI built from that commit. Stage 6a was
released as 0.5 on 2026-10-22, with the audit pass and the foundations pass: the tag `v0.5.0` is on
the commit "Release 0.5.0", and its GitHub release carries the Windows programs that CI built from
that commit. Stage 6b was released as 0.6 on 2026-10-27: the tag `v0.6.0` is on the commit
"Release 0.6.0", and its GitHub release carries the Windows programs that CI built from that
commit.

## Completed stages

| Stage | Milestones | Result |
|---|---|---|
| 0 — Foundations | M0.1 repository and ADR process; M0.2 Nix environment; M0.3 workspace, lints, crate skeletons; M0.4 dependency policy and CI | Done. CI green on the first push to GitHub (2026-10-12). |
| 1 — Core and platform → 0.0.1 | M1.1 logging and diagnostics; M1.2 frame timing; M1.3 lifecycle and headless mode; M1.4 window; M1.5 input; M1.6 clean shutdown | Done; released as 0.0.1 (tag `v0.0.1`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 2 — GPU layer → 0.0.2 | M2.0 Windows in CI; M2.1 GPU device without a window; M2.2 offscreen triangle and reference image; M2.3 the triangle in the window; M2.4 frame pacing; M2.5 debug markers, robustness, release | Done; released as 0.0.2 (tag `v0.0.2`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3a — First renderer → 0.0.3 | M3.1 GPU resources for a renderer; M3.2 the render crate; M3.3 directional light and material; M3.4 textures and samplers; M3.5 a glTF scene and camera control; M3.6 statistics, GPU timings and the dev overlay; M3.7 Windows check, documentation, release | Done; released as 0.0.3 (tag `v0.0.3`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3b — Physically based renderer → 0.1 | M3b.1 GPU layer for multi-pass rendering; M3b.2 HDR rendering and tone mapping; M3b.3 physically based materials; M3b.4 light types; M3b.5 shadows; M3b.6 image-based lighting; M3b.7 bloom; M3b.8 render graph; a pass over the known debt; M3b.9 Windows check, documentation, release | Done; released as 0.1 (tag `v0.1.0`), with the Windows examples on its GitHub release. Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3c — A first game | M3c.1 skeleton; M3c.2 guards, detection and the objective; M3c.3 night, feel and the Windows check; M3c.4 findings for the next stages | Done; no release of its own: its game, *Nightwatch*, is never released, as *Hunna Wood* replaces it in Stage 4a ([ADR-0067](adr/0067-hunna-wood-replaces-nightwatch.md)). The stage each of its findings goes to is in [ADR-0057](adr/0057-what-the-first-game-asks-of-the-next-stages.md), and the two that became Stage 3d in [ADR-0056](adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md). |
| 3d — Anti-aliasing and the shadows of spot lights | M3d.1 multisampling in the GPU layer; M3d.2 anti-aliasing in the renderer; M3d.3 spot lights' shadows: what each tile sees; M3d.4 spot lights' shadows: drawn and read; M3d.5 *Nightwatch* and the Windows check; M3d.6 a smoother shadow filter; M3d.7 what each cascade sees; M3d.8 the cascades read; M3d.9 *Nightwatch* at 3840 × 2160 | Done; no release of its own: what it delivers is released with 0.2. Its decisions are in [ADR-0056](adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md) and ADR-0058 to [ADR-0065](adr/0065-reading-the-cascades.md); the cost of its scene pass on Direct3D 12 with each shader compiler, which made DXC ship beside the Windows programs, is in [ADR-0066](adr/0066-shipping-dxc-beside-the-programs.md). |
| 4a — Cooked assets → 0.2 | M4a.1 *Hunna Wood*'s skeleton; M4a.2 asset identity; M4a.3 images; M4a.4 glTF; M4a.5 the sample scene through the pipeline; M4a.6 the zone's sources; M4a.7 *Hunna Wood* by day; M4a.8 night; M4a.9 Windows check, documentation, release 0.2 | Done, released as 0.2 on 2026-10-15. Its decisions are in [ADR-0067](adr/0067-hunna-wood-replaces-nightwatch.md) to [ADR-0071](adr/0071-cooked-textures.md); the zone's budgets are set by the test that checks them, in `games/hunnawood/src/zone.rs`, and what was left to refine went to Stage 4b's M4b.8. |
| 4b — Live assets | M4b.1 a shared GPU and a task pool; M4b.2 the zone loads while frames go on; M4b.3 recovering from a lost GPU device; M4b.4 *Hunna Wood* goes on after a lost device; M4b.5 materials that change in place; M4b.6 hot reload of cooked data; M4b.7 hot reload of shaders; M4b.8 refining *Hunna Wood*; M4b.9 Windows check, documentation, close | Done on 2026-10-16; no release of its own: what it delivers is released with 0.3, at the end of Stage 5a. Its decisions are in [ADR-0072](adr/0072-stage-4b-live-assets.md) to [ADR-0077](adr/0077-hot-reload-of-shaders.md). The owner checked the recovery from a lost device by hand at its close: in the Windows virtual machine, from the dev overlay and by a forced TDR, and in a KDE Plasma session after a real reset of the GPU (see [Known debt](#known-debt)). |
| 5a — The world → 0.3 | M5a.1 the world: entities and components; M5a.2 the hierarchy and the transforms; M5a.3 what the world draws; M5a.4 *Hunna Wood* on the world; M5a.5 reflection; M5a.6 any component listed and edited; M5a.7 serialization with format versioning; M5a.8 the world round-trips; M5a.9 saving and resuming a run; M5a.10 Windows check, documentation, release 0.3 | Done, released as 0.3 on 2026-10-17 with Stage 4b. Its decisions are in [ADR-0078](adr/0078-stage-5-in-two-halves.md) to [ADR-0088](adr/0088-saving-and-resuming-a-run.md), with the measurements of the world's storage, transforms and frame in ADR-0082 to ADR-0084; its save fixture, now `games/hunnawood/fixtures/save-0.3.toml`, is set aside by later versions as a save of another layout ([ADR-0090](adr/0090-stage-5b-scenes-and-prefabs.md)), and how long CI took with each crate it added is in the [Notes](#notes). |
| 5b — Scenes and prefabs → 0.4 | M5b.1 scenes placed in a world; M5b.2 a world written back as a scene; M5b.3 saves and hot reload with placements; M5b.4 the binary encoding of documents; M5b.5 scene files cooked, typed components, prefabs in the `world` example; M5b.6 *Hunna Wood*'s zone made of prefabs; M5b.7 Windows check, documentation, release 0.4 | Done, released as 0.4 on 2026-10-18. Its decisions are in [ADR-0089](adr/0089-scenes-and-prefabs.md) to [ADR-0095](adr/0095-cooked-scene-files.md), with the measurements of spawning scenes in ADR-0091; its save fixture, `games/hunnawood/fixtures/save.toml`, was made again at M6a.7b in the layout of saves per scene, which refuses those of 0.4 ([ADR-0125](adr/0125-saves-per-scene.md)); the zone is made of prefabs placing its land and six lanterns. The audit of 0.4 that followed it is the audit pass, in the next row. |
| The audit pass and the foundations pass | P1 saves and identities that never mislead; P2 a damaged file gives an error, never a crash; P3 the loss of the GPU device and the main loop; P5 the build, CI, and measurements tracked from one commit to the next; P4 the measured quick wins; P6 the foundation decisions | Done on 2026-10-20, released with 0.5. The engine was audited at 0.4, without changing it, for correctness, measured performance and architecture against the games it aims at; [ADR-0097](adr/0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md) sums up what it found and records the owner's decisions, and [ADR-0096](adr/0096-the-files-the-engine-reads.md) is the threat model that ranks its defects. P1 to P5 fixed them ([ADR-0101](adr/0101-saves-and-identities-that-never-mislead.md) to [ADR-0106](adr/0106-the-measured-quick-wins.md)), but B4-F01, which M6a.2 fixed; P6 decided what the editor freezes ([ADR-0107](adr/0107-the-execution-model.md) to [ADR-0115](adr/0115-what-the-gpu-layer-gives-the-editor.md)). Each finding was described in full in this plan until 0.5 (`git show v0.5.0:docs/PLAN.md`); the audit's report and its tests are kept outside the repository; what neither pass fixed is in the [known debt](#left-by-the-audit-of-04); the owner's checks by hand of P3 and P5, in the Windows virtual machine on 2026-10-27, passed. |
| 6a — What the editor stands on → 0.5 | M6a.1 frustum culling and instancing; M6a.2 the engine's task pool, and copies to the GPU within a budget; M6a.3 what the world knows of its changes; M6a.4 a frame that owns what it draws; M6a.5 the phases of a frame, game time, and a game as a simulation and a presentation; M6a.6 the development channel and the MCP server; M6a.7a driven components, keys and created entities written into levels; M6a.7b saves per scene and adoption; M6a.8a views apart from the renderer; M6a.8b the settings of a view; M6a.9 asset requests; M6a.10a drawing for the editor; M6a.10b picking; M6a.11 Windows check, documentation, release 0.5 | Done, released as 0.5 on 2026-10-22 with the audit pass and the foundations pass. Its decisions are in [ADR-0116](adr/0116-stage-6-in-two-halves.md) to [ADR-0130](adr/0130-picking.md), its measurements in [`measurements.md`](measurements.md), under the milestones that measured; its save fixture is `games/hunnawood/fixtures/save.toml`, of the layout of saves per scene ([ADR-0125](adr/0125-saves-per-scene.md)). The owner checked each milestone that changed what shows by hand, on Linux, and the whole in the Windows virtual machine at M6a.11, which found four defects fixed before the release. |
| 6b — The engine's interface → 0.6 | M6b.1a the commands and undo; M6b.1b the editor driven by an agent; M6b.2 text in several writing systems; M6b.3a the canvas; M6b.3b text drawn, and the display list; M6b.4a elements and layout; M6b.4b style sheets and themes; M6b.5a input and focus; M6b.5b what agents see; M6b.6a text edited, and what the platform lacks for it; M6b.6b screen readers; M6b.7a the widgets of a panel; M6b.7b the look's foundations; M6b.7c layers; M6b.7d rich fields; M6b.7e what the editor tells and finds; M6b.8a the editors and the world panel on the engine's interface; M6b.8b the overlay over the games' views; M6b.9 Windows check, documentation, release 0.6 | Done, released as 0.6 on 2026-10-27. Its decisions are in [ADR-0131](adr/0131-stage-6b-and-stage-6c.md) to [ADR-0154](adr/0154-floating-panels-and-held-keys.md), its measurements in [`measurements.md`](measurements.md), under the milestones that measured. Each milestone was described in full in this plan until 0.6 (`git show v0.6.0:docs/PLAN.md`); what it left is in the [known debt](#known-debt). The owner chose that the interface blends in sRGB's encoding, at M6b.3a, and Geist as the font of the editor's text, at M6b.3b; checked each milestone that changed what shows by hand, on Linux and for several in the Windows virtual machine, Orca and Narrator reading the examples; and checked the whole in the Windows virtual machine at M6b.9, which passed. |

The details of each completed milestone (what it delivered, how it was checked, what was measured)
are in the history of this file and in the commits that implemented it; the measurements that
justify a decision are also in its ADR.

## Known debt

Limitations known and accepted for now. The ones that a contributor could take on alone are also
GitHub issues, linked from their entry; this list stays the reference.

- **What a load reads waits in staging memory until its copies are submitted**
  ([ADR-0118](adr/0118-the-task-pool-and-the-copies-to-the-gpu.md)). The renderer's uploader sends
  at most a budget of bytes per frame, and nothing holds the loads back when they read faster: a
  load of many hundreds of megabytes keeps them in the CPU's memory while it lasts. The asset
  requests ([ADR-0128](adr/0128-asset-requests.md)) rank the loads and stop those that nothing
  holds, but hold none back.
  How many of the loads' tasks run at once, and the budget, were measured on the owner's RX 6900 XT
  alone; each renderer has an uploader of its own, and two renderers send twice the budget.
- **The development channel scripts keys only, and captures windowed runs only**
  ([ADR-0122](adr/0122-the-development-channel.md)). A tool puts keys down and lets them go, not
  the mouse nor text, which an agent gives an interface through its elements' actions instead
  ([ADR-0143](adr/0143-what-agents-see-of-the-interface.md)); a run without a window has no image
  to capture; the measurements that the channel gives are the frames' times, and the reports that
  a game gives, its views' statistics among them;
  an MCP server drives one game at a time, until M6c.4; a component that refers to an entity
  without a persistent identity is read, the reference written as no entity, `""`, and written back
  so, no longer naming it. A window's image is created with the copy-source usage wherever the system
  offers it, at every frame, whether or not a tool captures it: what that costs
  a driver is not measured. Each later milestone exposes its system to the channel
  ([ADR-0116](adr/0116-stage-6-in-two-halves.md)).
- **The first frame that draws shadows costs more, and more still while a scene loads**
  ([ADR-0074](adr/0074-uploading-textures-without-holding-up-frames.md)). It makes the renderer
  create its shadow atlas and its frame again; during a load, the GPU, busy with the load's copies,
  presents that frame late, past 33 ms in some runs, measured before M6a.2. M6a.2 holds the copies
  to a budget once a frame is drawn, but not those before the first frame, which that frame waits
  for; it was not measured again. *Hunna Wood* draws its hero, and so its first shadows, from its
  first frame. Creating the atlases with the renderer would remove it, at a cost in
  memory for every program, shadows or not.
- **The zone costs up to 8 s per frame on lavapipe**, the CPU driver of the checks, where the RX
  6900 XT takes under 2 ms: most of it goes to the four shadow cascades of the sun, each 2048 ×
  2048 texels, through which thousands of overlapping leaf cards are drawn with a shader that
  discards. The checks run the game for one frame by day, one at night, two around a lost device
  and one resumed from the save fixture. The oaks' cards made smaller and four times as many at
  M4b.8 changed it little: the day's shadow pass took 6.0 s
  instead of 6.2 s, in a release build. Leaves that cast no shadow in the farther cascades would
  lower it, to measure with views of the zone.
- **`cargo test` leaves out the tests of the cooked zone**: they need the zone fetched and cooked,
  so they are ignored (`#[ignore]`); `scripts/check.sh` and CI's `windows` job fetch and cook the
  zone, then run them (`cargo test -p hunnawood -p hunnawood-editor -- --ignored`). The agents'
  sessions through the MCP server, the six session tests of `slime-mcp`, are tests apart
  (`test = false`), which `scripts/check.sh` runs on Linux alone.
- **Frames are not synchronized with the display's refresh** (ADR-0021; issue
  [#1](https://github.com/HoneySlatt/SlimeEngine/issues/1)). With Mailbox, the default, the
  schedule alone paces frames: on a 60 Hz display, an image is now and then shown twice while the
  next one is dropped. On a display of 144 or 165 Hz, the default 60 frames a second last 2 or 3
  refreshes in turn, which shows as judder in motion: a game can raise `AppConfig::max_frame_rate`
  to the display's rate, which the engine does not report, or choose `PresentMode::Fifo`, which
  shows one image per refresh but can stall as the next entry says. Aligning frames on the
  compositor's frame callbacks is to reconsider when a renderer draws real scenes.
- **Without Mailbox, presenting can still stall the loop.** Windows that do not offer it, and
  applications that choose `PresentMode::Fifo`, wait for the display: one frame per second for a
  hidden window on niri. In a headless Weston with lavapipe, `acquire` waits for Weston's frame
  callbacks for a time that depends on the machine's load: 120 frames of the `window` example took
  15 s on 2026-10-12 (release build, waits of 130 ms on average), 9 s and 25 s in other
  measurements the same day, against 2 s with Mailbox.
- **A hidden window keeps drawing** at the scheduled rate (2 to 3 % of one CPU core on the
  development machine): the engine cannot tell on Wayland whether its window is visible. winit
  reports occlusion on X11, which the engine ignores: under a compositing manager, Xwayland
  included, the X server reports windows as visible anyway.
  On Windows, a window covered by another keeps drawing too; a minimized one, of size zero, draws
  nothing (M3b.9).
- **wgpu's errors panic, in release builds too** (ADR-0016): a use of wgpu that it rejects is
  treated as a bug. Before games ship, logging the error and dropping the frame is to be
  considered. Running out of GPU memory is not such a use: creations return
  `GfxError::OutOfMemory`, and memory that runs out where no caller hears of it gives the device up,
  which stops the run (ADR-0044). The engine avoids the rejections that the environment can
  cause: a window larger than the device's largest image is drawn at that size (`Surface::resize`),
  and a shader or pipeline that the driver rejects is returned as an error (`GfxError::Shader` or
  `GfxError::Pipeline` on Direct3D 12; on Vulkan, wgpu treats the driver's failure as a lost
  device, `GfxError::DeviceLost`); a window's surface that the graphics API refuses to configure
  counts as lost (ADR-0025). The refusal of a configuration is not tested: nothing the tests
  control makes a conformant driver refuse one.
- **On Linux, a game goes on after a real reset of the GPU only if the compositor does**
  (ADR-0075). A reset reaches every program that uses the GPU. Mesa ends the ones that did not ask
  to be told of it, as niri, Hyprland and COSMIC do not: on 2026-10-15, programs that hung the
  owner's RX 6900 XT made the amdgpu driver reset it twice, and niri ended each time, taking the
  session with it. KDE Plasma's KWin recovers from resets: on 2026-10-16, after a reset forced on
  the owner's machine in a Plasma session, *Hunna Wood* opened a new device 70 ms after it found the
  loss and drew its zone again 2 s later, its run kept, while the terminal it was started from and
  Plasma's panel ended. On Windows, the desktop's compositor survives resets, and the game went on
  after a forced timeout detection and recovery (TDR) in the owner's virtual machine.
- **A lost GPU device is replaced only for the applications that ask for it** (ADR-0075; issue
  [#2](https://github.com/HoneySlatt/SlimeEngine/issues/2)): the others stop with an error, and
  since winit allows one event loop per process, carrying on means starting the process again. An
  application that asks drops and creates its GPU objects again itself, as *Hunna Wood* does. A
  real driver reset can only be checked by hand, as it was at Stage 4b's end (above). A restore
  that fails in a windowed run is not tested: nothing makes one fail on purpose there, nor, so,
  that `run` then returns the application's error first (ADR-0103).
- **Three behaviours of wgpu 30 are worked around, and not reported to its authors**
  ([ADR-0103](adr/0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md)): wgpu holds a
  device invalid before it reports the loss, without a way to ask, and reports the use of objects
  created in between as a misuse; `Device::poll` panics on a lost device; on Vulkan, wgpu panics
  when it destroys a swap chain whose image work in flight still holds. The engine asks with a
  buffer of 4 bytes after each creation, waits for the GPU by submitting empty work, which wakes
  later after the GPU finishes than a poll would (`Gpu::wait_idle` says how much), and releases a
  surface's last image before its swap chain. P3 asks the owner's agreement to report them.
- **In capture tools, the sky pass shows as a second `scene`.** A pass is named after the target it
  draws into, and the sky pass draws into the scene's high-dynamic-range target, after the scene
  pass (`declare_frame` in `crates/slime-render/src/renderer.rs`); only its pipeline is named `sky`.
  Seen in RenderDoc on Direct3D 12 (M3b.9).
- **Frames pause on Windows while the user moves the window, holds its title bar or a border, or
  opens the system menu**: the system then runs a loop of its own, in which no redraw arrives and
  the engine's loop does not wake, so frames and the check for termination signals pause until it
  ends (ADR-0020; the move was measured in M2.4). Keeping frames going would mean asking for
  redraws from inside each frame and waiting there for the schedule.
- **wgpu-hal logs an error when an adapter cannot present to the window**, even though the engine
  then uses another one: seen with RADV in a headless Weston, where only lavapipe can present.
- **No title text in client-side decorations.** On compositors that do not draw decorations
  (GNOME), the title bar has no text, because winit's text renderer depends on an unmaintained
  crate (ADR-0011).
- **Input not reported yet** (issue [#3](https://github.com/HoneySlatt/SlimeEngine/issues/3)):
  touch, touchpad gestures, gamepads, dropped files. The window cannot go fullscreen. Input methods
  and the pointer's shapes came with M6b.6a, the title changed with M6c.1a, the mouse's raw motion
  and the cursor held with M6c.1b
  ([ADR-0157](adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md)); on Windows, winit 0.30
  reports as raw motion the positions that a virtual machine's tablet or a remote desktop gives.
  M6c.7 brings files dropped on Windows and X11, Stage 7 gamepads; files dropped on Wayland wait
  for winit 0.31 ([ADR-0155](adr/0155-the-editors-design.md)).
- **A compositor that stops answering blocks the engine.** Before the first frame, winit waits for
  the window's first configuration without a timeout, so a termination signal is noticed only when a
  second one ends the process; after the `stop` methods, releasing the window or the surface can
  block too (seen with a Weston stopped by SIGSTOP).
- **Logging off or shutting down Windows ends a game without its `stop` methods.** Windows tells a
  window of it with end-session messages, which winit does not report, and sends no console event to
  a process that has loaded `user32.dll`, as every engine process has
  (`crates/slime-app/src/signal.rs`): what a game saves in its simulation's `stop` is lost then.
  *Hunna Wood* saves its run there since M5a.9: the run since the last F5 is lost when Windows ends
  the session (ADR-0088). A fix needs a new dependency, which ADR-0081 leaves to a later decision.
- **WARP's shader compiler crashes the process, now and then, when several threads compile
  shaders at once**, even on separate devices: an access violation (`STATUS_ACCESS_VIOLATION`)
  that other projects trace to `d3d10warp.dll`, and that no test can catch. Seen once in the
  Windows CI job, on 2026-10-13, in the texture tests of
  `slime-gfx`, after about ten green runs; other projects that test on WARP report the same crash,
  and that running the tests one at a time avoids it, which the Windows job does since, and which
  `CONTRIBUTING.md` asks of contributors on WARP. The engine's own code is not involved.
- **Client-side decorations run `dbus-send` twice when the window is created** (winit's
  sctk-adwaita, on compositors that do not draw decorations), each waiting up to 100 ms when the
  settings portal does not answer; the error they log when no session bus or portal answers is
  hidden by the default log filter (ADR-0019).
- **Mipmaps of textures made in code are computed on the CPU when they are created**
  (`Gpu::create_texture`, whose documentation gives the cost), on the calling thread, in work
  proportional to the texture's pixels (ADR-0032): a level of tens of large textures waits seconds.
  A caller with levels of its own gives them to `Gpu::create_texture_with_levels` (ADR-0039), as
  cooked textures do, whose levels the cooker computed (ADR-0071).
- **Cooked bytes are the same from one cook to the next on one platform, not across platforms**
  (ADR-0071): the cooker computes the sRGB curve with the platform's mathematical functions, which
  may round differently on Linux and Windows. Tests compare decoded values across platforms; a
  table of the curve's values would make the bytes the same everywhere.
- **A cook reads and hashes every source file at every run, and reads every cooked file it
  reuses**, and cooked files that no source makes any more stay in the output directory (ADR-0071,
  ADR-0102): nothing records a source's size and time to skip unchanged ones, which is to measure
  on *Hunna Wood*'s zone, nor cleans the output, where each change that `slime-cook watch` cooks
  leaves the files it replaced (ADR-0076). The rustdoc of `slime_cook::cook` measures what reading
  the cooked files costs.
- **A cooked file damaged within its ranges is not found** (ADR-0102): the cooker cooks again a
  file deleted, emptied, cut short or that its reader refuses, and a game refuses such a file with
  an error, but texels or numbers changed within their ranges draw wrong, without a word.
  Checksums may come beside the rule, with archives (ADR-0096).
- **Two watches repeat the same polling** (ADR-0077): `slime_assets::ManifestWatch` and
  `slime_render::ShaderWatch` each read their files every half second and tell a failure once,
  with the same dozen lines, over what each reads. No crate that both reach suits a type for both
  yet.
- **Every offscreen render target and depth target can be sampled** (ADR-0033): they all carry the
  usage that allows it, which may cost some compression on some GPUs. Measured at M3b.5 on an RX
  6900 XT (RADV), with two builds that differ only by it: no cost (the scene pass took 43.9 µs
  with it and 44.0 µs without at 1920 × 1080, ADR-0033's correction). Other GPUs are not measured.
- **A pipeline that does not fit its pass, or a target sampled by the pass that draws into it,
  panics only when the commands of its submission are finished** inside `Gpu::submit` (or
  `Gpu::render`, `Gpu::render_depth`), with wgpu's message naming the pass, rather than at the
  faulty call (ADR-0033, ADR-0041). For the renderer's frame, the render graph refuses a pass that
  samples what it draws into before wgpu sees it.
- **On X11, losing the connection to the X server ends the process** with exit status 1, inside
  the X11 library (Xlib). `EventHandler::exiting`, the `stop` methods and the return of `run` do not
  happen, although ADR-0010 promises that they are called once the `start` methods have been. winit
  installs no IO error handler. Installing one (`XSetIOErrorExitHandler`, libX11 1.7 or later)
  would need a new direct dependency such as `x11-dl`. On Wayland, a compositor that dies ends the
  loop cleanly.
- **Without libxkbcommon, the windowing library panics** rather than returning
  `PlatformError::Connect`. On X11 it panics at once, also without libxkbcommon-x11 or without the
  X server's XKB extension. On Wayland it panics when the compositor first reports a keyboard,
  which can be in the middle of a run. Turning it into an error would mean probing the libraries
  before winit does.
- **A termination signal received after `run` has returned no longer ends the process by itself**
  (ADR-0012). `ctrlc` cannot remove the handler that `run` installs. A first signal received then
  only stops later runs, and logs that the run stops after the current frame; only a second one
  ends the process, with exit code 130. This matters for a program that keeps working after
  `run`, for example under a service manager, which waits out its stop timeout. Ending the process
  at once when no run is active would amend ADR-0012.
- **A window frame dropped without being presented keeps one of the window's images on Vulkan.**
  wgpu 30 discards such a frame with wgpu-hal's `discard_texture`, which does nothing on Vulkan. The
  image stays acquired until the surface is configured again, and once none is left,
  `Surface::acquire` waits one second and returns `None`. `SurfaceFrame` documents this as a misuse,
  and nothing reports it. The engine presents every frame it acquires, a panic in
  `Presentation::render` aside, whose frame the surface releases when it is dropped, as it releases
  an image that a lost device refused to present
  ([ADR-0103](adr/0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md)). A `Drop` that
  presents was rejected: it would show a black or half-drawn frame instead of skipping it, and
  present during unwinding.
- **An oversize window's surface is configured again at every frame on X11.** A window wider or
  taller than the device's largest image (the adapter's, 16 384 pixels on lavapipe and the RX
  6900 XT since M6a.4 asks for the adapter's limits) gets images of that size, smaller than the
  window. On X11, Mesa's Vulkan drivers mark such a swapchain suboptimal, and every image of it too.
  `Surface::acquire` treats each suboptimal image as a reason to configure the surface again, at
  the same clamped size, so every frame waits for the GPU to finish and rebuilds the window's
  images. The frames are correct, and only `debug` logs show it; Wayland and Direct3D 12 are not
  affected. Not setting `stale` in the `Suboptimal` arm of `Surface::next_image` while `oversize`
  is set is not enough: measured, the `Outdated` branch then configures the surface again. Such
  windows are rare since the device asks for the adapter's largest image, and were not measured
  again. To measure, before and after: start Xvfb with a screen wider than 16 384 pixels; with
  `WAYLAND_DISPLAY` unset, `DISPLAY` set to it and lavapipe, run the child of the `oversize`
  scenario of `crates/slime-app/tests/windowed_lifecycle.rs`, its window widened past the largest
  image, with `SLIME_LOG=wgpu_core=debug`; count wgpu-core's `configuring surface with` records
  (about one per frame without the fix, one with it) and note the frame times.
- **Termination signals are ignored for the rest of the process when `ctrlc` cannot start its
  thread.** `ctrlc` installs its handlers, then starts the thread that waits for what they catch.
  When that thread cannot start (a limit on threads reached when `run` installs the handler), the
  handlers stay installed and SIGINT, SIGTERM and SIGHUP (Ctrl+C and Ctrl+Break on Windows) are
  ignored: only SIGKILL, or closing the console on Windows, ends the process. The engine logs a
  warning that says so and gives the system's error. Restoring the default dispositions would need
  a direct dependency on libc or nix.
- **Any `trace` directive in `SLIME_LOG` makes wgpu build its per-command trace records.**
  tracing-subscriber sets the `log` crate's maximum level from the most verbose directive. With
  `slime_app=trace`, wgpu-core then formats and filters out about five records per draw while it
  encodes a pass, which counts in the caller's span (`render` in a windowed run): measured with
  `RenderStats::cpu_time` for a scene of 1000 instances, 0.38 ms more per frame, 35 %, with
  `slime_app=trace` than with `SLIME_LOG` unset. Alternatively, profile with
  `perf record -g` and look for `tracing_log::LogTracer::log`, `EnvFilter::enabled` and
  `wgpu_core::resource::Labeled::error_ident`. Capping the `log` level needs `log` or
  `tracing-log` as a direct dependency (ADR-0006). Span close lines are also written while the
  enclosing span is entered, so `frame`, `render` and, with `slime_gfx=trace`,
  `RenderStats::cpu_time` include them: take timing baselines with `SLIME_LOG` unset.
- **On Windows with Vulkan (`WGPU_BACKEND=vulkan`), an acquire that waits more than one second
  loses the device.** wgpu-hal 30.0.1 waits on a fence after `vkAcquireNextImageKHR` hands out an
  image, on Windows only, and maps a timeout of that wait to `DeviceError::Unexpected`, which
  wgpu-core treats as the loss of the device. A GPU frame longer than about one second, or a
  compositor that holds the window's images that long (a covered window, a locked session), then
  stops the run with `GfxError::DeviceLost`, with a reason that blames the driver; on Linux the
  frame is skipped instead. Not reproduced: in the owner's Windows 11 virtual machine with an AMD
  driver, the `scene` example kept running with its window covered for six seconds and with the
  session locked (2026-10-13). To report to wgpu: that wait's timeout should give
  `SurfaceError::Timeout`, as its other waits do.
- **Seen from below, the sample scene's walls and crates flicker where they touch the floor.** Their
  bottom faces lie in the floor's plane, and since M3b.3 the floor is double-sided, so from below
  both are drawn at the same depth (z-fighting). The `scene` example keeps its camera above the
  floor, but another viewer of the asset sees it. Lifting them by a few millimetres in
  `examples/assets/make_scene.py` fixes it, at the cost of rendering the glTF references again.
- **A mouse button can stay down on Wayland when the compositor takes the pointer away during a
  press without taking the keyboard focus.** winit 0.30 reports only `CursorLeft` then, and
  `Translator` (`crates/slime-platform/src/event.rs`) releases held buttons only when the focus is
  lost. Releasing them on `CursorLeft` would be right on Wayland only: on X11 and Windows the
  cursor leaves the window during an ordinary drag.
- **On Windows, the first of two held Shift keys let go of is released late.** Windows sends a
  single key-up for both Shift keys, so the engine releases the first one only when the second
  goes up. Windows 11 marks the second Shift's press as a repeat, so neither its press nor its
  release is reported (seen in the owner's virtual machine on 2026-10-13). Polling `GetKeyState`,
  as GLFW and SDL do, would remove the delay and report both keys, but needs `windows-sys` as a
  direct dependency.
- **After a dead key, a press and its release report different logical keys.** On Wayland and
  X11, a character key that does not combine with a pending dead key is reported as
  `Key::Unidentified` with no text; when the keys combine, the press is the composed character and
  the release the plain one. winit exposes no better value; rebuilding one would need libxkbcommon
  called directly. Documented in `KeyEvent::logical`.
- **A key pressed just after the window gains the focus can be swallowed on X11 and Windows.** winit
  reads which keys are down when it handles the focus event, not when the focus changed. A key
  pressed in between counts as held before the focus and is not reported until it is pressed again;
  on X11 the gap covers the `start` methods when the window manager focuses the window while it
  runs. The translator cannot tell such a press from X11's unmarked first auto-repeat. On Windows,
  trusting `KeyEvent::repeat` would close the gap, at the cost of a rule that differs between
  platforms; left as documented.
- **No test covers the empty submission of frames without a window image.** When
  `Surface::acquire` gives no image, it submits an empty batch, so that wgpu releases the writes
  a game makes outside `Presentation::render`. Without it, 2000 frames of a surface resized to
  zero, each writing 64 KiB to a uniform buffer, grew the process by about 120 MiB (lavapipe in a
  headless Weston, 2026-10-13), measured by a program kept out of the repository. The
  `draws-nothing` scenario of `crates/slime-app/tests/windowed_lifecycle.rs` checks the same for
  frames whose image nothing draws, not for frames without an image.
- **A GPU time can read 0 once when the timestamp counter wraps.** On adapters whose timestamps
  have 36 valid bits (Intel's Vulkan driver, about once every one to one and a half hours), a
  timed pass that runs across the wrap ends before it starts. `GpuTimer::latest` then gives 0 for
  that pass until the next time replaces it. wgpu does not tell how many bits are valid, so the
  wrapped difference cannot be recovered. Dropping such a sample (`end.checked_sub(start)` in
  `crates/slime-gfx/src/timer.rs`) is a one-line change if it ever matters.
- **Exposed light beyond 64 × 65 504 is clamped** (ADR-0035, ADR-0037, ADR-0039, ADR-0040). The
  scene and sky shaders write exposed light divided by 64 into the `Rgba16Float` target, and light
  above 64 × 65 504 as 65 504, the largest half, rather than leave it to the driver, which may store
  an infinity that the tone curve turns into NaN; lavapipe and RADV clamp it themselves, so that no
  test can show either clamp. The sun's disc, 1.47 × 10⁹ nits for the sample's sun, is clamped from
  an exposure of 2.85 × 10⁻³, six and a half stops above the sample's; emission above
  64 × 65 504 / exposure, and any light once the exposure is raised about 13.5 stops above the
  sample's, are clamped too, and the bloom spreads them with the energy they keep. A sky's radiance
  is at most 65 504 nits before the exposure: brighter zenith, horizon or ground is refused, and a
  brighter glow kept within it. Divided by 64, exposed light below 64 × 2⁻¹⁴, about 0.004, is
  stored in half precision's subnormals, in steps of 64 × 2⁻²⁴: black on an 8-bit display, whose
  darkest step is about 0.007 exposed; an output of higher precision would show its steps.
- **Every fragment evaluates every light of the scene** (ADR-0037). The scene pass has no light
  culling: its cost grows with the number of lights, about 20 µs per light for a screen of 1920 ×
  1080 fragments on an RX 6900 XT (2026-10-13), 338 µs for 16, the most a scene holds, and 10 to 20
  times more on an integrated GPU; a light's range saves no work. More lights would need clustered
  or tiled culling and a storage buffer, a measured decision of Stage 9a (ADR-0052).
- **Point lights cast no shadows, and light what objects hide** (ADR-0037; the first directional
  light that casts shadows casts them, ADR-0038, and so do spot lights, ADR-0058). The sample's ring
  light, a point light, reaches 4 m, and gives little beyond a metre. The shadows of point lights
  come with Stage 9a (ADR-0052).
- **Every instance of a scene costs CPU time at each frame** (`Renderer`,
  [ADR-0117](adr/0117-frustum-culling-and-instancing.md)). Frustum culling and instancing make a
  frame's draws and its GPU work follow what each pass sees, but the renderer keeps nothing from
  one frame to the next: at each frame, it places and tests every instance's box against the
  camera's view and against every cascade and spot light, on one thread, ranks every instance's
  material and mesh, sorts those drawn, and packs and sends their values, about 0.2 µs per
  instance of 100 000 cubes on the owner's machine (`render.instances.100k`,
  [`measurements.md`](measurements.md)). A render scene kept from one frame to the next, with
  stable keys, and culling and draws decided by the GPU come with Stage 9a (ADR-0097).
- **The tests that run out of GPU memory under a budget depend on the machine's memory in use**
  (`slime_gfx::MEMORY_BUDGET_ENV_VAR`). On lavapipe, the budget is a share of the memory that the
  whole machine uses, so that the shares at which such a test runs out of memory as it means to lie
  in a window, a few shares wide, that what else runs moves by a share or two between runs. On
  2026-10-22, `a_view_whose_new_size_does_not_fit_keeps_no_targets` failed twice locally and once in
  CI, run at the window's lower end. These tests now run halfway through the window, which probes
  find first (`slime-gfx`'s `tests/common/memory_budget.rs`), and that one alone in its binary,
  `slime-render`'s `tests/oversize.rs`. Each probe of that test makes a renderer: it takes about
  15 s on lavapipe, and a minute or more on WARP. Memory that moves by a third of the window while
  one runs still fails it.
- **A picking draws all that its view sees, and picking by the CPU ignores masks**
  ([ADR-0130](adr/0130-picking.md)).
  A picking's pass draws every instance that the view's scene pass draws, not those of its area
  alone, in the frame that it is drawn; a view draws one picking per frame. Picking by the CPU
  passes through nothing that a mask cuts out, reads each mesh whole, at its first cast in a world
  that names it, to know its box, which a bounds-only read of the cooked data would spare, and reads
  from the cooked data that its `Picker` was made with, not again when it changes. Lines are not
  picked. *Hunna Wood* does not pick yet: a tool's picking in it is refused. Scripted input has no
  mouse, so that an agent picks through the MCP server's `pick` tool, not by clicking.
- **What a client draws into a view is lines and instances in the scene, and lines over the image**
  ([ADR-0129](adr/0129-drawing-for-the-editor.md)). The scene's depth resolved for an outline comes
  with M6c.2b, and instances over the image, which need a depth of their own, with M6c.3's handles.
  A line that lies on a surface may be hidden by it in places; blended surfaces are drawn in no
  particular order until Stage 9a sorts them. The `world` example selects with Tab from the
  keyboard alone: scripted keys reach a game's input, which its simulation reads, not its
  presentation's events. How long `Renderer::set_shaders` takes was measured before the
  unlit kind's pipelines and the lines'.
- **A page of meshes keeps the ranges of dropped meshes until new ones fill them or it empties**
  ([ADR-0117](adr/0117-frustum-culling-and-instancing.md)). Nothing moves a mesh to pack its
  page: a game that drops half its meshes keeps their pages' memory while one mesh stays in each.
  Dropping a mesh frees no GPU memory until its page empties: the asset cache counts its pages, and
  a held mesh keeps the releasable meshes of its page ([ADR-0128](adr/0128-asset-requests.md)).
  Capture tools name a page's buffers, not its meshes.
- **The asset cache's requests leave work and memory behind**
  ([ADR-0128](adr/0128-asset-requests.md)). Two batches loading at once that both read a texture the
  cache does not hold yet upload it each, and the cache keeps the first. The copies that a stopped
  load handed to the uploader still go to the GPU, at its rank. A texture is dropped whole, its
  finest levels with its coarsest, until textures stream by their levels at Stage 10a
  ([ADR-0113](adr/0113-asset-requests-residency-and-priorities.md)). *Hunna Wood* has a frame
  builder for each time of day, each with its sky's environment, and the one that does not draw
  keeps the handles of its last frame until it draws again. The tests drop meshes from one page
  only, which the squares that they load share: no test drops the least recently held of several.
- **Cascades are fitted to spheres around slices of the view** (ADR-0064). Each covers its slice
  whichever way the camera looks, which keeps its texels steady, but a wide view makes the spheres
  large: in the first game, *Nightwatch* (deleted at M4a.7), with the single map's 30 m, the texels
  near the player would be about half as wide as its texels, not a quarter, by ADR-0064's study;
  that game's moon cast its shadows up to 16 m to make them finer. Fitting the cascades to what the camera sees at each frame,
  or virtual shadow maps, is left to Stage 9a.
- **Shadows shift, and thin objects close to a surface lose theirs (peter-panning)** (ADR-0050).
  The normal offset that keeps surfaces from shadowing themselves looks a point up 4.5 texels × the
  sine of the light's incidence off its triangle (ADR-0063, ADR-0065): at the default distance,
  under a light 45° from the normal, 1.2 cm in the first cascade and 18 cm in the last, and 26 cm
  there at grazing. A shadow shifts by up to that much, and a sheet lying closer than that above a
  floor casts nothing on it.
- **Every draw of the scene pass runs a shader that can discard fragments** (ADR-0035). The `MASK`
  alpha mode discards in the one fragment shader that every material uses, opaque materials with a
  cutoff of 0. Some GPUs then test depth after the shader rather than before, for opaque draws
  too, which costs overdraw. On lavapipe, opaque materials drawn without `discard` did not make the
  scene pass faster ([ADR-0106](adr/0106-the-measured-quick-wins.md)); on a GPU, not measured:
  compare the scene pass's GPU time on a scene with much overdraw against a build whose opaque
  materials use a fragment entry point without `discard` (six pipelines instead of three).
- **Edges that a masked material cuts inside a triangle stay aliased** (ADR-0059). Multisampling
  smooths the edges of triangles, but the scene shader runs once per pixel, and the `MASK` alpha
  mode keeps or discards all of a pixel's samples at once. Smoothing those edges, alpha to
  coverage, behaves differently from one driver to the next, which image tests would see. The leaf
  cards of *Hunna Wood*'s trees and its ferns are masked.
- **The sky's light is neither shadowed nor occluded** (ADR-0039). A surface under an object, or
  inside a box, receives the whole sky, but for its material's occlusion texture: no ambient
  occlusion (screen-space effects are outside the stage), no reflection probes, and mirrors reflect
  the sky, never the objects around them. An interior is lit by the whole sky through its walls:
  until then, draw it without an environment, or with a sky of lower radiances, and light it with
  point and spot lights. Khronos's Sponza shows it (M3b.9): its arcades are lit by the sky as its
  open courtyard is, in the same blue.
- **Glass is drawn opaque.** The importer leaves out glTF's `KHR_materials_transmission` and
  `KHR_materials_volume`, and warns that it does: the renderer neither lets light through a surface
  nor blends one (Khronos's FlightHelmet and ABeautifulGame, M3b.9). Transparency is left to
  Stage 9a (ADR-0052).
- **One GPU, an AMD, and two CPU drivers have run the engine.** The image tests' tolerances
  (ADR-0030), the measurements of this plan and the samples' looks come from an RX 6900 XT, with
  RADV on Linux and AMD's driver on Windows, from lavapipe and from WARP: NVIDIA's and Intel's GPUs
  are untried, and stay so for now, the owner having no other machine (ADR-0097). The ratios that
  scale the hardware tiers' budgets to the RX 6900 XT are estimates from public benchmarks until
  the tiers' machines are measured ([ADR-0114](adr/0114-hardware-tiers-and-the-gpus-limits.md)):
  a GPU rented in the cloud, a Steam Deck, or contributors would end it. The handheld's ratio, a
  tenth, may be generous for the interface's canvas, whose cost lies more in writing pixels than in
  computing them, the Steam Deck writing them about twenty times slower than the RX 6900 XT
  ([ADR-0137](adr/0137-the-canvas.md)). The RX 6900 XT also draws the owner's desktop, whose work
  moves its figures ([`benches/README.md`](../benches/README.md#reading-the-figures)).
- **The environment is computed on the CPU, in every process that draws a sky** (ADR-0039): 23 ms
  in a release build on the owner's 16 threads, 162 ms on one core, and the BRDF table once (15
  ms); about eight times longer in a debug build, which every test that draws a sky with a gradient
  or a sun pays, most in `slime-render`'s `tests/environment.rs` and the examples' `tests/gltf.rs`,
  whose every load of a file computes the sample's sky: 9 s and 13 s, timed by hand at P5, of the
  workspace's 79 s of tests on lavapipe, on a machine of 4 logical CPUs
  ([measurements](measurements.md) holds the total). A sky that changes needs a new environment:
  no time of day. To measure: `cargo test --release -p slime-render --lib -- --ignored
  --nocapture`, against the budget of M3b.6. Not yet scheduled: to move to the cooker, or to
  compute passes if skies change at run time; if the tests' time matters first, building
  `slime-render` optimized in debug builds would shorten it, at the cost of stepping through it in
  a debugger.
- **The sky's sun is described twice in the renderer's own API** (ADR-0039). A game that draws
  with it gives the same `DirectionalLight` to `Sky::sun`, for the disc and the glow, and to
  `Scene::lights`, for its light and its shadows; two different lights show a disc where no light
  comes from. The `scene` example takes both from the file's first directional light. A world gives
  it once ([ADR-0084](adr/0084-what-the-world-draws.md)), as *Hunna Wood*'s does.
- **A node renamed in Blender loses what saves held of it** (ADR-0079, ADR-0087). A scene's entity
  is known by its node's name: loading a save made before the rename sets aside what it held of
  the node, and reports it. *Hunna Wood*'s zone script names every node and fails when Blender
  numbers one, so that only a change of the script renames a node; the saves made before it set
  aside what they held of it, or are saves of another layout. Identities that a scene file keeps for
  a former name wait for a game that has players (ADR-0089).
- **Some meshes of *Hunna Wood*'s zone are named with Blender's numbers** (`wooden_lantern_01.001`
  in the lantern, `capital.004` in the land): the script checks the names of its objects, not of
  their meshes. The cooker derives a mesh's ID from its name, so that a later change of the script
  that numbers a mesh otherwise changes the mesh's ID. Naming the meshes changes the export, and
  waits for the next release of the mirror.
- **Blender's exports differ from one run to the next** (G55, low): the hero's, the lantern's on a
  machine with another number of threads, the four textures that Blender renders, and the sample
  scene's sphere. Blender computes the tangents of a mesh of more than 10 000 faces in parallel, in
  an order that changes; its renders write their date and duration in the PNG files' text; and its
  UV sphere orders its faces otherwise in each process. The hero kept on the mirror is the one the
  game reads. Running the scripts on one thread, with `--factory-startup` and a pinned version of
  Blender, and removing the PNG files' text, would make them reproducible.
- **A save cut short may load in part** (ADR-0087). A save is a document of sections: one cut at the
  boundary of a section is still a document, and loads what it holds without an error. *Hunna Wood*
  writes its saves whole or not at all (ADR-0088), as every game that writes saves must.
- **A save is text, of limited size** (ADR-0087, ADR-0102). Saving refuses a save larger, or of more
  values, than what `slime-serialize` reads, about 22 000 entities that have a name and a transform,
  a value counted for each `=`, `,`, `[`, `{` and `.` of the text, a float's point included; loading
  costs more than saving, much of it parsing the text: the rustdoc of `World::save` and
  `World::load` has the figures. *Hunna Wood*'s saves are far below the limit. The binary encoding
  exists since M5b.4, but saves stay text until a measurement asks (ADR-0089): a larger world needs
  saves in the binary encoding, or read in parts.
- **An entity that a run created, wrote into its level and destroyed since comes back when the run
  is carried into the file cooked again**
  ([ADR-0124](adr/0124-driven-components-keys-and-created-entities-in-levels.md)): the save no
  longer names it, and the file holds it. A save that names the written identities of what the run
  destroyed would settle it; the editor writes levels through its commands from M6b.1a, and play in
  the editor, M6c.4, settles it ([ADR-0155](adr/0155-the-editors-design.md)).
- **A game's preparation of a scene that a save spawns may change only the scene's entities**
  ([ADR-0125](adr/0125-saves-per-scene.md)): what it creates, under a key or as a placement, is the
  run's, which a save holds, and which loading the save creates before it prepares the scene, so
  that a preparation that creates an entity under a key fails, and one that places a prefab places
  it twice. *Hunna Wood* and the examples prepare without creating.
- **What a run created under a scene's entities outlives the scene when it is unloaded**
  ([ADR-0125](adr/0125-saves-per-scene.md)): it is taken out of them, at the world's root, rather
  than kept with what the world keeps of the scene, as an item dropped in a level would be.
- **What cannot be applied of what the world kept of a scene unloaded is logged, not reported**
  (`World::settle`): as when the scene changed since it was unloaded, at the `warn` level, where a
  save's load reports it to the game (`Loaded::set_aside`).
- **`Range::contains` stays public beside `Range::holds`**: two ways to check a number against a
  range, the first in `f64`, which only tests' helpers use. Every check of a field's range uses
  `Range::holds` (ADR-0101); the helpers can move to it, and `contains` go.
- **Files stay beside *Hunna Wood*'s save**: `save.toml.lock`, which locks the save while a copy of
  the game runs, and `save.toml.<process id>.new`, when the game is killed while it writes. Neither
  is read again, nor removed.
- ***Hunna Wood*'s lock holds a save by its path** (ADR-0101): two copies that reach one save by two
  paths, one through a symbolic link, take two lock files and do not exclude each other; on a file
  system without locks, the game neither resumes nor saves, and says so; a copy of 0.4, which takes
  no lock, still writes a save that a later copy holds.
- **A save that removes a component of a type that the program does not know sets it aside**
  (ADR-0101): `removed.components` names it, and loading reports that the entity holds no such
  component, where a value of an unknown type is a warning. Writing the save back loses the
  removal, which a later version that knows the type would have applied; *Hunna Wood* sets aside
  the saves of later versions by the version that they say, which covers it for the game.
- **Children and queries in another order after loading** (ADR-0087). A world loaded from a save
  holds its entities in other slots, and an entity's children in the order the save set them: what
  must go the same way after loading cannot depend on either order.
- **Spawning a large scene makes each component from its value** (ADR-0091 holds the figures; on
  the game's zone, at M5b.6, reading and resolving its scenes took 1.24 to 1.75 ms in a release
  build, twice what 0.3's one scene took). A scene resolves and spawns far slower than as many
  entities spawned with `World::spawn` (ADR-0083): each component is made from its value, and each
  entity's identity is a list of texts, held twice. ADR-0089 orders the remedies, inside the world:
  typed components copied rather than made from values, identities that share their parts, scenes
  prepared on another thread.
- **A world written back writes what it holds of what spawning set aside** (ADR-0092). A
  reference, or a parent, that a scene file wrote into a placement whose prefab no longer had its
  entity is written as the world holds it, no entity or the root: what it named is lost, as a save
  loses it. An entity of a placement moved out of the place that its prefab gives it is written in
  that place, and the children of a placement's entity in its prefab's order, then the file's; both
  are reported: a scene file has no parents for a placement's entities, nor an order among them.
  M6c.6 decides whether a file says them, with the new layout of scene files
  ([ADR-0155](adr/0155-the-editors-design.md)).
- **`World::write_scene` drops a scene file's comments** (ADR-0089): it writes the canonical form;
  the editor puts them back (`slime_serialize::keep_comments`,
  [ADR-0134](adr/0134-the-editors-core.md)), within the limits that the editor's core gives below.
- **The cooked glTF scene is a format apart from the binary encoding** (ADR-0089, ADR-0095). A
  glTF file's scenes are cooked into a format of their own, `slime_assets::CookedScene`, and only
  the typed properties of their nodes into the binary encoding: cooked scenes have two formats, and
  two readers. The renderer's scene loader, the `scene` example and the image tests read the format;
  moving it onto the binary encoding would make `slime-render` depend on serialization, and waits
  for a reason.
- **After a lost device, what is hidden loads only once shown** (ADR-0084): the cache loads what is
  drawn. In *Hunna Wood*, a key taken or a door opened before the loss shows a few frames after R
  shows them again.
- **Any change of *Hunna Wood*'s cooked data makes a new game of its scenes.** With `--watch`, the
  game makes a new game of the scenes before it knows whether a scene changed, a texture saved in an
  image editor included (`take_assets` in `games/hunnawood/src/main.rs`): about 2 ms, for a tool of
  development.
- **A zone loaded again with `--watch` changes in parts.** Its entities change at once, the meshes,
  materials and textures that the cache holds when its reload ends, and new meshes once their batch
  is loaded; *Hunna Wood* swapped whole scenes before it ran on the world.
- **The tests that edit every leaf of every component are written twice** (ADR-0085): once in
  `slime-world` for its components, once in *Hunna Wood* for the game's, about fifty lines that
  choose another value for a leaf. One copy would need a test-support crate, or a feature of
  `slime-world` that only tests enable; neither is worth it yet.
- **An edit through reflection reads the whole component again** (ADR-0085): a field that its type
  skips takes its `Default` value, and a rotation is edited whole, as `Transform`'s check refuses a
  coordinate edited alone. The dev overlay's editors edit a rotation, a vector and a color whole.
- **Rough metals reflect the sky with multiple scattering compensated, the lights without**
  (ADR-0039, as the Khronos glTF Sample Viewer): a white metal of roughness 1 reflects all of a
  uniform sky, but its lights keep glTF's single-scattering BRDF, under which it would reflect
  31 % of that sky; rough metals are darker in the sun than in the shade, relatively.
- **A roughness between two prefiltered levels reads a blend of their lobes** (ADR-0039): the four
  levels are 0.25 apart, and a lobe in between is approximated by mixing the nearest two, the split
  sum's usual approximation; each level is within 2.2 % of its own lobe's average.
- **The image tests use up to 75 of the 131 pixels that RADV may have off by more than 8**
  (ADR-0030, ADR-0054): the glTF scene's front view 75, on the wall's mortar joints seen at a
  grazing angle (76 before its textures were cooked); `shapes` 28, on edges and the metal spheres'
  reflections of the sky (49 at M3b.6, before the bloom; 33 before the shadow map followed the
  camera and the output was dithered). A change that adds fine or reflective detail may exceed it;
  measure it again then.
- **Four passes per submission is a number measured on one machine** (ADR-0041). On the owner's
  RX 6900 XT (RADV), whose memory that the CPU can map is a 256 MiB window, one submission of a
  whole bloomed frame stalled in the kernel once the desktop's programs and a test program filled
  that window to 241 MiB (a mean 2.4 ms, 8 ms in one frame of ten, at 1920 × 1080); submissions of
  2 to 6 passes did not, 8 did. The cause in the kernel is not established; Direct3D 12 and other
  GPUs are not measured, and a GPU with resizable BAR has no such window. To measure: the CPU time
  of `render` at 1920 × 1080 with the sample's bloom, for several values of
  `PASSES_PER_SUBMISSION` in `crates/slime-render/src/graph.rs`, with the window filled to
  several levels, by the desktop's programs or by a program that allocates device-local,
  host-visible memory (on Linux, `mem_info_vis_vram_used` in the GPU's sysfs directory tells how
  full it is). M6c.2a measures it so, the bloom on and off, since the editor's viewports meet the
  stall ([ADR-0155](adr/0155-the-editors-design.md)).
- **The render graph trusts what a pass says it samples from imported targets** (ADR-0041). The
  groups that sample transient targets are built from the declarations, but the scene pass's group
  holding the shadow atlases is built by the renderer: a pass that binds an imported target must
  declare it, or the graph orders it without knowing. The views after the first of a frame read
  the spot lights' atlas, which the first drew, without declaring it: they rely on the order of the
  submissions, one view after the other ([ADR-0126](adr/0126-views-apart-from-the-renderer.md)).
- **An orthographic camera's shadows reach the sun's shadow distance from its eye**
  ([ADR-0126](adr/0126-views-apart-from-the-renderer.md)): a map drawn from higher above the ground
  than that distance shows none, and its four cascades are nearly as large, the image's width and
  height setting them more than their depths (`slime_render::Projection`). Fitting an
  orthographic camera's cascades to what it sees is to decide when a game draws such maps with
  shadows.
- **The views of a frame draw the same lights**
  ([ADR-0126](adr/0126-views-apart-from-the-renderer.md)): beyond the 16 that the renderer draws,
  `slime-world-render` keeps those nearest one point that the game gives, so that views far apart,
  as two players' halves of a split screen in two places, share the lamps of one of them. Stage 9a's
  method for many lights decides how a view gets its own.
- **A view whose frame changes shape at every call reports no tone-mapping or bloom time**
  (ADR-0041, ADR-0126, ADR-0127). Each change of shape (the size of its targets, its anti-aliasing,
  a pass starting or stopping, by the scene or by its settings) builds the view's frame again with
  new timers, and the last submission's times come back during the next call, by then read from
  newer timers. A window's edge dragged no longer does it, its size held until it is let go, nor
  does a view drawn into rectangles of two sizes in turn, which draws one of them stretched: use
  one view per size.
- **A view's new size waits for its next frame after `RESIZE_DELAY`**
  ([ADR-0127](adr/0127-the-settings-of-a-view.md)): a view drawn only when something changes, as an
  editor's viewport may be, keeps its image stretched until it is drawn again `RESIZE_DELAY` after
  its size last changed. The editor's viewports draw again then, from M6c.2a
  ([ADR-0155](adr/0155-the-editors-design.md)).
- **A render scale reads the scene's light bilinearly**
  ([ADR-0127](adr/0127-the-settings-of-a-view.md)): below 1 the image is blurred, and between 1 and
  2 each pixel averages the four texels nearest its centre, missing others. Stage 9a's temporal
  upscaling is the sharper way; a pass of a sharper filter was rejected.
- **Two modes of debugging** ([ADR-0127](adr/0127-the-settings-of-a-view.md)): the normals and the
  shadows. M6c.2b adds base color and light alone, and a wireframe of the selection; a wireframe of
  the whole scene waits until `PolygonMode::Line` is measured against lines along a mesh's edges,
  which M6a.10a's drawings give a client to draw ([ADR-0155](adr/0155-the-editors-design.md)).
- **Tiny, very bright sources blink in the bloom** (ADR-0040). Without a firefly filter (Karis's
  average dims the sun's halo and makes it flicker, measured), a highlight smaller than a pixel
  blooms as its coverage of the pixel changes from frame to frame. Temporal anti-aliasing, or a cap
  on the light a pixel brings to the chain, belong to Stage 9a (ADR-0052).
- **A pixel whose light is not a number spreads through the bloom** (ADR-0040). The renderer refuses
  values that are not finite in meshes and materials, and the skies' radiances; it leaves out a
  light that it cannot draw, as the frame builder does a sun, and bounds the light that each light
  and the sun's disc give a point (ADR-0102). Not everything that reaches the shaders is checked: a
  normal of zero length, which `MeshData` asks callers not to give, can make its pixels NaN in the
  high-dynamic-range target, and so can, at an exposure of 0, an emission so strong that, added to
  the light of the scene, it is not finite; the bloom averages them into its levels, and from there
  into every pixel within reach of their halo, a blot where one surface was wrong. Dropping such
  light before the bloom would keep it to its pixels.
- **The bloom has one shape** (ADR-0040): every level the same weight, down to a side of 8 texels;
  no radius, threshold or tint for a game to set. A window resized across a power of two adds or
  removes a level: the glow's reach changes a little while the edge is dragged.
- **Light near the right and bottom edges blooms a little more or less** (ADR-0040). Where an
  output's side is not a multiple of 2^levels, the coarse levels' last texels straddle the edge: a
  pixel's halo there holds 0.83 to 1.37 of its light at ten sizes measured, up to 1.9 at the far
  corner (1023 × 1023, 2047 × 1151), within 1.011 at 1280 × 720 and about 1.1 at 1920 × 1080; inside
  the image and at the top and left edges, its light exactly. To measure: the CPU model of
  `tests/bloom.rs` at the size in question. A fix would pad the chain beyond those edges.
- **glTF minification filters are approximated.** `slime-gfx` ties mip blending to the
  minification filter and gives every texture mipmaps, so only `NEAREST_MIPMAP_NEAREST` and
  `LINEAR_MIPMAP_LINEAR` are exact (documented in the importer, `crates/slime-cook`). Revisit when
  an engine-side user needs a separate mipmap filter or a base-level-only read.
- **A restored entity that had a base takes its values as its base**
  ([ADR-0134](adr/0134-the-editors-core.md)), as every entity that a scene spawned has: what a run
  changed of it before it was deleted is no longer told apart from what its scene gave it, and a
  save made afterwards does not hold it. The editor never saves its world; a game that deletes and
  restores such an entity through the world's commands loses that difference.
- **The editor's journal is handed to the system at each step, not forced to the disk**
  ([ADR-0134](adr/0134-the-editors-core.md)): a crash of the editor loses no step, a crash of the
  system may lose the last ones.
- **Undoing a deletion in the editor copies what it recorded**, and recording it reads every
  component, of every entity of the world, of a type that may refer to an entity
  ([ADR-0134](adr/0134-the-editors-core.md)): `world.undo.1k` measures the world's commands on a
  subtree of 1 000 entities, of types that refer to none, not the editor's step.
- **The editor does not follow the prefabs that change on disk while it edits**
  ([ADR-0134](adr/0134-the-editors-core.md)): its library holds the cooked scenes as it opened them.
  M6c.6 follows them.
- **A comment within a value written on several lines is lost** when the editor writes the level
  again, but for the end of the value's first line (`slime_serialize::keep_comments`).
- **The canvas draws nothing turned or scaled** ([ADR-0137](adr/0137-the-canvas.md)): the items'
  transforms, whose place they keep, come with Stage 7. An image is read at its first level, so that
  one drawn much smaller than it is shows steps, and its edges are neither rounded nor smoothed
  between two pixels; an image of colors darkens where an opaque texel meets a transparent one of
  another color, its texels filtered before they are multiplied by their alpha. A shadow is an
  approximation, a few hundredths of its alpha from the box blurred exactly. A canvas holds at most
  `MAX_CANVAS_ITEMS` items. The interface's budget is checked on the RX 6900 XT alone, through
  ADR-0114's ratios. Writing a canvas's items into its buffer costs the CPU in proportion to their
  number, item by item ([`docs/measurements.md`](measurements.md), M6b.3a).
- **The interface's images are textures that the program gives** for the keys of its display list
  ([ADR-0139](adr/0139-text-drawn-and-the-display-list.md)): cooked images that the asset cache
  holds come with their first user, M6c.7's asset browser. A draw that asks for more glyphs than
  the atlas's budget holds leaves some out, and a glyph larger than a page is never drawn; one that
  meets glyphs no draw has asked for rasterizes them on the thread that draws, a few milliseconds
  for a menu's first draw ([`docs/measurements.md`](measurements.md), M6b.3b).
- **The interface's text has one style per text** ([ADR-0136](adr/0136-the-interfaces-text.md)):
  spans of several styles in one text come with their first user. An italic or a weight that a
  family lacks is drawn with its closest face, its slant or boldness not made up. A face whose
  colors are only of `COLR`'s second version, as some emoji fonts of systems, draws no color, swash
  drawing the first version alone. A word wider than its box, on a line before the last one shown,
  overflows the box rather than being cut. The cache's memory is an estimate fitted once to a
  measurement, of texts not cut; a hit copies its key; and fonts registered are never let go.
- **CI's jobs and their limit of 20 minutes**: until ADR-0144, the cache of the cooked zone was
  keyed by the whole `Cargo.lock` and root `Cargo.toml`, so a milestone that added a dependency,
  even one that the cooker does not use, cooked the zone again in each job. At M6b.2, that took the
  `windows` and `check (windowed)` jobs past their limit by under a minute, on the first run, at
  M6b.3a, `check (windowed)` by 3 minutes, and at M6b.3b by under a minute; at M6b.4a, `windows`
  came within half a minute of it. At M6b.4b, the cooker's own sources changed, and `check
  (windowed)` passed the limit by 40 seconds; at M6b.5a, they changed again, and it took 18 minutes
  45 seconds. At M6b.5b, `Cargo.lock` changed, and with the agent's session on the interface,
  `check (windowed)` took 20 minutes 28 seconds and `windows` 20 minutes 51 seconds.
  [ADR-0144](adr/0144-the-cooked-zones-key-in-ci.md) keys the cooked zone on the packages that the
  cooker is built from: a dependency added to another crate no longer cooks it again, but a change
  of the cooker's sources or of its packages still does. M6b.6a was the first milestone to add a
  dependency since, arboard, in `slime-platform`: the zone was restored from its cache, and the
  four jobs came back within the limit, `windows` the longest at 18 minutes 9 seconds. M6b.6b added
  AccessKit's adapter and its 52 crates on Linux, and the longest job, `windows-release`, took 15
  minutes 27 seconds: the runs that the windowed part holds grow with each milestone. At M6b.7b,
  `slime-color` changed, and the zone was cooked again: `check (windowed)` took 21 minutes 55
  seconds and `windows` 21 minutes 39 seconds. At M6b.8b, with the zone restored from its cache,
  the game's agent session through the overlay made `check (windowed)` take 23 minutes 31 seconds:
  the owner decided to split the windowed part again, into a third job, as
  [ADR-0123](adr/0123-the-linux-checks-in-two-jobs.md) split the checks, the limit of 20 minutes
  kept ([ADR-0155](adr/0155-the-editors-design.md)). The two runs after the release of 0.6 failed,
  the game's session past its own limit of 15 minutes. M6c.0 split the checks into `headless`,
  `windowed` and `game`, and made the session draw the zone without shadows: on `a199369`, the three
  Linux jobs took 13 minutes 35 seconds, 4 minutes 42 seconds and 4 minutes 14 seconds, the zone
  restored from its cache; cooking it again adds about 3 minutes to `check (game)` alone. What
  remained `windows`: 19 minutes 43 seconds on `a199369`, 20 minutes 23 seconds on `ebc16f2`. Its
  later steps ran the image tests, the GPU timing tests and the tests that read the zone with `-p`,
  whose features differ from the workspace's: on `a199369`, building again for them took 2 minutes
  2 seconds, 50 seconds and 53 seconds. The first two now print from the one run of the tests, and
  `windows` took 11 minutes 20 seconds on `bb4552b`. The tests that read the zone still build again,
  about a minute: run over the workspace, `--ignored` would also run the measurements that other
  crates ignore, meant for release builds.
- **The interface's tree** ([ADR-0140](adr/0140-the-tree-of-elements.md)): a frame walks every
  element of its roots, to find what to draw again and to gather the display list, even when nothing
  changed. A list shows the rows of a new range one frame late, when it first shows or when it
  scrolls after its builder ran. A text in a root laid out from right to left keeps its own
  direction, a Latin paragraph starting at its box's left. No element is placed out of its parent's
  flow but in a layer of its own ([ADR-0150](adr/0150-layers.md)). The marks of what changed hold an
  element's style, layout, paint and meaning ([ADR-0146](adr/0146-screen-readers.md)).
- **Input and focus** ([ADR-0142](adr/0142-input-and-focus.md)): buttons, fields and widgets alone
  hold the focus, buttons and widgets alone are pressed, and boxes that scroll
  and lists scroll with the wheel; a list's rows are
  not selected; a press on what the interface draws is taken whether or not it does anything, from
  what its last frame drew, so that a game's interface that lets presses through draws nothing
  where they fall, and the wheel with it; the shadow that a box casts takes them over the box, not
  over the blur that spreads past it. A root is modal from when it is added. The interface's actions reach no command of the editor or of a game but through
  the program. The hover follows what a layout moved under a still pointer a frame late, and the
  focus leaves an element disabled or hidden at the next frame; the focus moved does not scroll its
  element into view. A layout of another script than the Latin one matches a chord of a letter by
  the key's place, and a chord of a digit or a sign by its meaning alone. The example `interface`
  scrolls its list with time, which the wheel does not move. Touch comes later. M6c.5 selects a
  list's rows and scrolls the focus into view.
- **Text edited** ([ADR-0145](adr/0145-text-edited.md)): a double or triple click selects no word
  or line, and the caret does not blink. A field of one line leaves the up and down arrows to the
  focus, shows no bar of its own as its text scrolls, lays its text out one caret's width within
  its content box, so that the caret shows at either end, and does not scroll up and down, so that
  a text taller than its box is cut above and below. Escape in a field leaves the focus, which
  commits what the user typed rather than giving back what it held. Each edit lays the field's
  whole text out again, at a cost that grows with its length; a field undoes up to 100 edits, and
  forgets them when the program gives it another text. A field has no length that it holds at
  most, no text hidden as a password's; a field of numbers no bounds, without a prefix
  is dragged only while it does not hold the focus, and reads a full stop alone as its decimal mark,
  whatever the user's language. The input method's window is placed beside
  the caret as the last frame drew it, and is turned off and on again, meant to end a composition
  that the focus left, which no input method was checked with yet; a text that the program gives
  while an input method composes, which the field reads without the composition, is left aside, so
  that a field clears itself once the composition ends. The clipboard holds text alone; on
  Wayland, it needs a compositor that gives the protocol of the clipboard's managers, wlroots',
  niri's and Hyprland's among them, and falls back on X11 through XWayland elsewhere. An agent sets
  a field's value whole, and does not type in it key by key.
- **Widgets** ([ADR-0148](adr/0148-widgets.md)): a check box has no third state, mixed; a slider
  lies across alone, and shows neither ticks nor its value; tabs are neither closed, moved nor
  scrolled when the bar is too narrow for them; two panels are two, of no least size, and a third
  child is folded; scroll bars show over the content as long as it does not fit, and have no
  arrows. Whether a section is open, which tab is chosen and where a handle stands are the
  interface's alone: they tell the program nothing, and an element that a builder removes forgets
  them. The editor's look has no italic faces; its fonts hold no Greek; it is built into the
  program, not cooked: the dev overlay carries it into a game's development build, and the game's
  build for shipping leaves it out. M6c.5 gives a check box its third state; M6c.8 moves and
  closes tabs, and gives panels a least size.
- **The look's foundations** ([ADR-0149](adr/0149-the-looks-foundations.md)): a theme's values are
  typed only where a rule uses them, so that a name that holds a value of the wrong type is refused
  where a rule names it, not where the theme gives it; the color functions are four, and work on
  whole colors, not on their lightness, chroma or hue alone. Transitions move colors, opacity and a
  section's height, not sizes, places nor what a layout gives; they go on only while the program
  gives the interface its time, and a section that opens is laid out twice in the frame that
  starts it. An opacity multiplies each primitive's alpha, rather than drawing the element apart
  and blending it whole: overlapping parts of a translucent element show through each other, until
  Stage 7's layers. A shadow is not placed on whole pixels. On Linux, the system's dark or light
  preference is not read: winit reads no desktop's settings, and the portal of the desktop's
  settings would need a new dependency; Windows reports it.
- **Layers** ([ADR-0150](adr/0150-layers.md)): a menu's separators are boxes of the class
  `separator` that the look styles, not a kind of their own; a menu has no letters that jump to an
  item, and no way to cross its siblings towards an open submenu: resting on another item on
  the way closes it. A list to choose from does not scroll its options, so that a
  list taller than the window is cut; Tab within an open list moves among its options rather than
  closing it. A tooltip shows for the keys' focus alone, not the program's, and is not placed
  beside the pointer. A floating panel has a least size of its own, no docking, and does not keep
  its place against a window that shrinks, where it is placed within the window but keeps what the
  user left. A style sheet reads a theme's value in place of every value but a text: the look names
  its icons by their codepoints in its sheet, not in a theme. The bar of menus takes Alt let go
  alone whenever an interface holds one, which a game whose interface holds a bar loses as a key of
  its own. M6c.8 docks floating panels. Since M6c.1a, a root is named in paths by its key, not by
  its number among the roots, which a raised floating layer changed.
- **Rich fields** ([ADR-0151](adr/0151-rich-widgets.md)): a press on a focused field's prefix that
  does not drag places its caret at its start; a field's prefix and its unit's symbol are drawn
  whatever their width, cut by nothing; a bar of progress without a share that less motion stills
  stays still until it is drawn again; the look's fallbacks for a magnifier and a cross, without a
  font of icons, are one disc. Units are of three families, lengths, angles and times, not masses,
  speeds or colors. The box of a section's header is placed by the interface after taffy lays it
  out, which placed it from the wrong box. A content box takes its padding from the left whatever
  the root's direction, so that an asymmetric padding is mirrored wrongly from right to left, a
  drift found while placing the fields' marks, which allow for it.
- **What the editor tells and finds** ([ADR-0151](adr/0151-rich-widgets.md)): the palette finds
  commands by letters in order, without forgiving a letter typed wrong (it scrolls past 12 since
  M6c.1a); the sheet of shortcuts takes no focus, so that a screen reader
  says nothing as it opens; a notification's mark that closes it is named `close` in English; no
  check runs a screen reader on the live regions of notifications; the stack of notifications turns
  from its corner as a layer at a point does, which holds for a stack wider and taller than its
  margin, as every notification is.
- **What agents see** ([ADR-0143](adr/0143-what-agents-see-of-the-interface.md)): a tool reads an
  interface only in a windowed run, from its last frame, and each ask walks the whole tree. A path
  holds while the roots below its own stay, and a key's name that holds
  `/`, or that `#` and digits make, cannot be read back from one. A row of a list is found by its
  number, the names of its texts not gathered into its own, until M6c.5. Games link `slime-ui`
  through `slime-app`, whether they have an interface or not.
- **The dev overlay** ([ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md),
  [ADR-0153](adr/0153-tab-and-f6-in-a-games-interface.md)): a frame after an entity is renamed,
  spawned or despawned makes the world panel's list again, every name read and filtered, about
  7 ms at 100 000 entities ([measurements](measurements.md#m6b8-rx-6900-xt)), past the budget of a
  list's frame, until M6c.5's tick of names spares it the renames; the frames standing still,
  scrolled, or after the selection changed stay within it. Its panels keep the size that they open
  at, their content scrolling, where egui's fitted theirs. It holds its interface's tables of
  shortcuts, which a game's interface would share; a game loses F6 while its interface shows a
  root that holds an element that can take the focus, and F6 gives a panel's bar the focus first.
  Its look, the editor's, is built into the program: its themes are files of `slime-ui`, read as
  the program starts, not from the disk while it runs. No check of CI runs a screen reader on it.
- **Screen readers** ([ADR-0146](adr/0146-screen-readers.md)): no check of CI runs a screen reader
  nor Windows' UI Automation; CI reads and uses the `interface` example through AT-SPI on Linux
  ([ADR-0147](adr/0147-the-screen-readers-check-in-ci.md)); the owner checks Orca and Narrator by
  hand, on the `interface` example and on the `world` example's dev overlay. A modal dialog is not
  marked modal to screen readers, which find the elements below it taking nothing; a label's text gives no runs, so that a screen reader reads it whole; a
  field of numbers takes no increment or decrement, and nothing scrolls an element into view
  before M6c.5; the hover and the press are given to agents, not to screen readers, which AccessKit
  does not ask. A
  root's box that changes, a window resized, marks all it holds, and a modal root opened or closed
  makes the next changes whole. Each element's meaning is made again from the tree when marked, the
  boxes that only lay others out below it walked, and a field's runs laid out from its text, as it
  draws it. On Windows, which never says that no client reads any more, a window whose tree was
  asked for once is given an update after each frame until it closes. A text that its builder names
  gives screen readers its name, not its words; a caret at the end of a line that breaks within the
  text is placed at the start of the next, its side lost; screen readers scroll by an item or by
  what shows, not to an offset. `deny.toml` ignores three advisories of unmaintained crates that the adapter on Linux brings
  through zbus 5, zvariant, zbus_names and zvariant_derive, which zbus 6 holds: they go once
  AccessKit's adapter moves to zbus 6.
- **Style sheets and themes** ([ADR-0141](adr/0141-style-sheets-and-themes.md)): a field of the type
  `String` takes no named value. An error of type in a
  cooked style sheet or theme is placed by its byte and the path of its part, not by its line. The
  code cannot set a field of an element's style to its default against a rule. The styles made are
  kept up to a number of the elements' own styles for each set of rules (`Ui::set_style`); past it,
  an element's style is made again through reflection at each change, the slow way that
  [`docs/measurements.md`](measurements.md), M6b.4b, measures. Selectors hold no combinators but an
  ancestor's kind or class. A color is held whole in a part of a style by `slime-ui`'s own walk,
  beside reflection's, which holds a rotation whole: two ways of one thing, which a type's attribute
  of reflection would unify. An array, a list and an option of an element's own style replace the
  rules' whole, its height with its width.
- **The system's fallback is checked only where the system has fonts**
  ([ADR-0136](adr/0136-the-interfaces-text.md)): its test checks nothing, and says so, on a machine
  whose fontconfig, or whose lack of it, gives no font for Greek or Japanese, as CI's Linux runners
  may.
- **An editor's journal grows across recoveries** ([ADR-0135](adr/0135-the-editors-program.md)):
  each recovery goes on with the journal that it recovered, which only a start with nothing
  unwritten begins anew, and the history of steps outlives a stop only when something is
  unwritten. `editor_journal` fails once the journal is larger than the channel's largest message
  (`slime_serialize::MAX_SIZE`).
- **A selection made just before a crash is lost** ([ADR-0134](adr/0134-the-editors-core.md)):
  the journal writes a selection made apart from a step with the entry that follows it.
- **A journal whose last entry a crash of the system tore is not recovered**
  ([ADR-0135](adr/0135-the-editors-program.md)): the program stops, and `--discard` sets the
  journal aside with every step that it holds, those before the tear included, which only a journal
  mended by hand plays again.
- **Two editors of one level, each with a journal of its own, do not see each other's changes**
  ([ADR-0135](adr/0135-the-editors-program.md)): the second to write the level is refused, the file
  changed apart from it, and its changes stay in its journal alone.

### Left by the audit of 0.4

Defects that the audit of 0.4 confirmed and that neither the audit pass nor the foundations pass
fixes ([ADR-0097](adr/0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)), each with its
number, its rank on [ADR-0096](adr/0096-the-files-the-engine-reads.md)'s scale and the stage it
waits for. The audit numbered its findings so that a fix can name what it fixes: G01 to G56 are
groups of defects, B1 to B4 performance (the build, the world, the renderer, loading), each of their
findings numbered within its group, B4-F01 for instance, C1 to C7 architecture. A test that confirms
each is kept outside the repository, and comes in with its fix. The audit did not cover NVIDIA and
Intel GPUs, X11, a game outside the workspace, the delay from a key to the image shown, nor
determinism between Linux and Windows.

- **A mesh that a passing error failed is not tried again** (G16, medium; M6c.7). Since M6a.9,
  a mesh of an asset cache's request fails alone ([ADR-0128](adr/0128-asset-requests.md)), and is
  not asked for again until the cooked data changes, a passing error such as GPU memory running out
  included; a scene's load still fails whole. Trying a passing error again is to decide with the
  editor's asset browser.
- **On Windows, a `.meta` whose name differs from its source's by case only takes a new ID at every
  cook** (G20, medium; M6c.7), and `slime-cook watch` cooks again without end: the scan matches
  the two files by the file system's rule, which ignores case on NTFS, then looks the ID up by the
  exact path. Not run on Windows by the audit: it waits in the
  [list of checks by hand](#checks-by-hand-waiting-for-the-owner).
- **Importing a glTF file takes time quadratic in its named meshes** (G23, medium; M6c.7): 38 to
  57 s for 80 000 meshes, as each name is counted against the whole list.
- **The world's storage keeps 4 MB per component type and per million slots, and gives nothing back
  after destruction** (G28, medium; Stage 10a): each type keeps an index of 4 bytes per slot up to
  the highest slot that held it, and neither the index, the nodes nor the slots shrink
  ([ADR-0082](adr/0082-the-worlds-storage-and-handles.md)).
- **Masked foliage loses its coverage in its smaller mip levels** (G30, medium; Stage 9a): the
  cooker averages alpha from one level to the next, so that the share of texels above the cutoff
  falls in the levels drawn far away, down to none, and trees fade in the distance. Preserving
  coverage per level for textures that a masked material reads is to do before large exteriors.
- **`World::write_scene` writes, without an error, a level that cannot be opened again** (G31,
  medium; M6c.6): it never counts the entities that the file gives once its placements resolve,
  which the reader refuses beyond `MAX_ENTITIES`, nor what they take, refused beyond
  `MAX_SCENE_SIZE`.
- **A very deep hierarchy costs quadratic time** (G32, medium; M6c.5): spawning, loading, saving
  and each frame walk every entity's ancestors (`set_parent`'s search for a cycle,
  `World::unsaved`, `interpolated_transform`). M6a.3 made `World::is_hidden` read a flag.
- **Between two fixed steps, the children of a turning parent cut the arc's chord while their
  siblings stay on it** (G38, low; Stage 7): a child moved by its parent is drawn in a straight line
  between its two positions. Interpolating through the hierarchy is to decide before skeletal
  animation turns many hierarchies.
- **A string on several lines in a scene file checked out on Windows keeps its CRLF line ends**
  (G40, low; M6c.6): `.gitattributes` keeps `*.toml`, `*.meta` and `*.tsv` at LF, not `*.scene`,
  so that the value and the cooked bytes depend on the checkout.
- **A negative integer, or one too large, for an unsigned field gives a wrong message** (G42, low;
  M6c.6): "not an integer" or "not text", rather than its range.
- **The writer of documents accepts what the reader refuses** (G43, low; M6c.6): a reserved key
  `$…` at the root, and a field given twice, the second lost.
- **A camera target farther than about 2 × 10¹⁹ m draws nothing, without a message** (G44, low;
  later): `view_from_eye` squares the direction in `f32`.
- **`World::write_scene` loops without end when more than 1 679 616 entities need IDs from one
  base** (G45, low; M6c.6): it draws four base-36 characters until one is free.
- **`Registry::register_dynamic` accepts version 0, which the derive refuses** (G48, low; Stage 8):
  the values are written, then never read back, with a wrong message.
- **Two cooks in one process share their temporary files** (G49, low; M6c.7, before the editor
  cooks): named after the process, they separate processes, not threads, and one cook fails at
  random, naming a file that exists.
- **An output directory inside the sources breaks the next cook** (G50, low; M6c.7): cooked glTF
  scenes take the `.scene` extension of scene files, which the next scan reads as sources.
- **After a glTF file is moved with its `.meta`, the cooker's cache keeps the old path** (G51, low;
  M6c.7): the manifest names it, and warnings blame tables that are right.
- **Two `.meta` with one ID, one of them without a source, make the cook stop without saying which
  is the orphan** (G52, low; M6c.7).
- **`Manifest::from_bytes` refuses a valid manifest whose names are short** (G53, low; M6c.7): it
  assumes 45 bytes per entry where the least is 37.
- **With `--watch`, after the cooked data is cleaned, a reload that the game refuses leaves it
  unable to save** (G56, low; M6c.7, with cooking from the editor): `SceneLibrary::reload` forgets
  the changed scenes before
  the game accepts them, and the files of the old manifest are gone.

## Notes

- CI build cache: not added yet. With wgpu, the Linux job takes about 2.5 minutes and the Windows
  job about 5 (measured on the runs of 2026-10-12; it compiles the dependencies three times: for
  clippy, for the tests, and in release for the artifact), which is acceptable; a cache is to add if
  it grows much further. The dependencies of Stage 3a (glam, gltf, egui) were expected to add to
  it, and each milestone that added one recorded the new duration here. With glam and the render
  crate (M3.2, 2026-10-13): Linux 2 minutes 49 seconds, Windows 4 minutes 59 seconds. With gltf and
  egui (M3.6, 2026-10-13): Linux 3 minutes 22 seconds, Windows 5 minutes 42 seconds, and 6 minutes
  2 seconds once its tests ran one at a time. At the release (M3.7, 2026-10-13), over the ten
  green runs before it: Linux from 2 minutes 35 seconds to 3 minutes 38 seconds, Windows from
  3 minutes 12 seconds to 7 minutes 10 seconds; the Windows runner's speed varies from one run to
  the next more than the code changes between them. At 0.1 (M3b.9, 2026-10-14): Linux 5 minutes 40
  seconds, Windows 11 minutes 32 seconds, of which the tests take 4 minutes 31 seconds and the
  release build of the examples 3 minutes 24 seconds. The owner decided then to keep CI as it is,
  the repository being public and its minutes free, and to reconsider past 20 minutes. With
  `slime-world` (M5a.1, 2026-10-16): Linux 10 minutes 25 seconds, Windows 17 minutes 20 seconds.
  With `slime-color` and `slime-world-render` (M5a.3, 2026-10-16): Linux 12 minutes 33 seconds,
  Windows 17 minutes 26 seconds. With `slime-reflect` and `slime-reflect-derive` (M5a.5,
  2026-10-16): Linux 9 minutes 6 seconds, Windows 18 minutes 27 seconds. With `slime-serialize`
  (M5a.7, 2026-10-16): Linux 13 minutes 26 seconds, Windows 15 minutes 31 seconds. With no crate
  added since, at M5b.6 (2026-10-18): Linux 14 minutes 17 seconds, Windows 19 minutes 48 seconds,
  12 seconds under the limit. At P5 (2026-10-19), with the same work, from 13 minutes 38 seconds to
  21 minutes 54 seconds on Windows, as the runners' speed varied: the Windows job is split in two
  jobs that run in parallel (ADR-0081, ADR-0104), and every job keeps the cooked zone in its cache:
  on `2d71729`, the zone cooked again, Linux 13 minutes 4 seconds, `windows` 11 minutes 35
  seconds, `windows-release` 11 minutes 7 seconds; on `1cc469b`, the cooked zone in the cache, 9
  minutes 9 seconds, 10 minutes 26 seconds and 8 minutes 51 seconds.
