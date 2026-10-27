# 0155. The editor's design

- Status: Accepted; amends [0021](0021-frame-pacing.md) (a program may run its frames only when
  something asks for one), [0072](0072-stage-4b-live-assets.md) (notify, rejected for games, used
  by the tools), [0076](0076-hot-reload-of-cooked-data.md) (the cooker's watch told of changes by
  the system, polling as its fallback), [0089](0089-scenes-and-prefabs.md) (the editor opens a
  level that holds what the program does not know; the world updated in place at Stage 8),
  [0091](0091-scene-documents-and-placements.md) (a level names its parts in a table of their own),
  [0110](0110-how-the-editor-runs-a-games-code.md) (Play hands the game private copies of the
  scenes not written, and the editor's steps while it runs),
  [0112](0112-levels-saves-and-worlds-for-open-worlds.md) (how a level names its parts, and the
  layout of scene files changed once, with `EntityRef`'s),
  [0121](0121-a-game-as-a-simulation-and-a-presentation.md) (the editor's window reads the
  editor's core), [0122](0122-the-development-channel.md) (version 10 of the protocol: the editor's
  operations listed and run by name; an editor's requests waiting while a person drags; an MCP
  server that drives an editor and the game it plays; a game started with a token ending when its
  standard input closes), [0123](0123-the-linux-checks-in-two-jobs.md) (the Linux checks in three
  jobs), [0131](0131-stage-6b-and-stage-6c.md) (Stage 6c's milestones, their order and the
  owner's two checkpoints) and [0134](0134-the-editors-core.md) (a gesture made one step through an
  open step)
- Date: 2026-10-27

## Context

[ADR-0131](0131-stage-6b-and-stage-6c.md) makes Stage 6c the editor, released as 0.7, planned in
detail at Stage 6b's close, in an order that stays provisional until then. Its exit is Stage 6's: a
level of the game is built, saved and reloaded entirely in the editor, with undo and redo on every
operation, and the editor uses no private engine API.

Where the engine stood at 0.6:

- **The editor has a core and a program, without a window**
  ([ADR-0134](0134-the-editors-core.md), [ADR-0135](0135-the-editors-program.md)): commands that
  name entities by their persistent identities, steps undone, redone and journaled, the selection,
  a level of one scene file written back with its comments; the `editor` example and *Hunna Wood*'s
  editor package, which an agent drives through the MCP server. A presentation can only queue the
  world's commands (`Context::send`), each its own step: it can neither ask the core to undo, to
  select or to write, nor read the selection and the history.
- **The engine's interface** ([ADR-0132](0132-the-engines-ui-framework.md), ADR-0140 to
  [ADR-0154](0154-floating-panels-and-held-keys.md)) has widgets, layers, a menu bar, a palette of
  commands, notifications and floating panels, read by agents and by screen readers; the dev
  overlay's editors of reflected values and its world panel run on it
  ([ADR-0152](0152-the-dev-overlay-on-the-engines-interface.md)). It has no docking, no dialogs to
  open and save, no drag and drop between elements, no rows that are selected.
- **Views, picking and drawing for the editor**
  ([ADR-0126](0126-views-apart-from-the-renderer.md) to [ADR-0130](0130-picking.md)): a view draws
  into a target of its own, which the canvas shows; picking by the GPU and by the CPU; lines and
  unlit instances in the scene, lines over the image.
- **A frame runs at the scheduled rate** whether anything changed or not
  ([ADR-0021](0021-frame-pacing.md)); the platform gives no raw motion of the mouse, holds no cursor
  and drops no file; Play does not exist; a level is one file.

The owner chose on 2026-10-27 how the design was studied, as for the interface's framework
(ADR-0132). Four agents, each alone, read the repository and proposed a design from another
starting point: what other editors learned; the Rust ecosystem and the platforms; what fits the
engine as it is; and the work of the people who use an editor, a level designer, a programmer and
an agent. A fifth built prototypes outside the repository and measured them on the owner's machine,
an AMD Ryzen 7 5800X and an RX 6900 XT through RADV, whose memory that the CPU can map is a window
of 256 MiB (no resizable BAR), in release builds of Rust 1.99.0, with the owner's desktop and the
other agents running, which made its figures of the CPU up to two or three times slower than an
idle machine's. A synthesis reconciled them, and checked in the repository what they disagreed on.
The reports, the synthesis and the prototypes are kept outside the repository, as the audit's
report was. What they found:

- **Editors drew lessons that agree.** Play runs out of the editor's process in Godot and Fyrox;
  editors draw their viewports when something changes, not at every frame; panels in windows of
  their own work worst on Wayland, where a client cannot place its windows (winit's
  `set_outer_position` is not supported there). Godot 4.6 does show a game inside its editor on
  Wayland, by writing the compositor-like *embedder* that ADR-0110 describes, about 3 600 lines of
  C++: ADR-0110's premise holds, and now has a price.
- **A viewport costs the GPU little, and the CPU much on this machine.** The examples' yard in a
  viewport of 1276 × 839 took 0.13 ms of GPU, the zone at 1920 × 1080 0.33 to 0.83 ms, and the
  view's image composited by the canvas 8.6 µs. `Renderer::render` took 0.46 ms of CPU per view
  while the memory that the CPU maps had room, and 7 to 9 ms once the desktop had filled it, 29 to
  32 ms for four viewports: [ADR-0041](0041-render-graph.md)'s stall, still there, most of it from
  the bloom's submissions. A frame whose interface changed and whose view was not drawn again took
  0.8 to 0.95 ms, about 2 ms in the stall; `UiRenderer::draw` took 0.28 ms with nothing changed; on
  lavapipe, a frame with the view drawn took 84 to 96 ms.
- **A pick is answered two frames later** (three at worst), at 0.5 to 0.6 ms of CPU, 7.5 to 8 ms in
  the stall, since it draws the whole view: fine for a click, too dear for a highlight under the
  pointer at every frame.
- **A hierarchy of 100 110 entities is cheap when the editor flattens it and builds only the rows
  that show**: 3 µs standing still, 78 to 85 µs scrolled by a row. A rename followed through
  `World::structure`, which a name moves, cost 1.4 to 1.7 ms; through `World::changed::<Name>`,
  60 µs.
- **Play is far within ADR-0110's 3 seconds**: *Hunna Wood* gave control 0.42 to 0.69 s after it
  was started on its zone, caches warm, nothing built nor cooked; 8 to 15 s on lavapipe. A cook of
  the zone that changes nothing took 0.23 s.
- **Every request over the development channel took 82 to 97 ms**, whatever the frame rate:
  `slime-channel` writes a message in four writes on a socket where Nagle's algorithm waits for
  delayed acknowledgements. A client that writes once with `TCP_NODELAY` and acknowledges at once
  took 0.1 ms. A view of a running game through the channel is impossible until the wire is fixed.
- **The ecosystem.** transform-gizmo 0.11 cannot be a dependency: it brings egui's fonts, whose
  license `deny.toml` refuses, a second glam and 26 to 28 crates. winit 0.30 gives the mouse's raw
  motion everywhere, a locked cursor on Wayland and Windows but not on X11, and files dropped on
  Windows and X11 without the point where they fall; its 0.31, in beta, adds drops on Wayland, and
  `accesskit_winit` has no release for it yet. rfd loads D-Bus's library at run time, which the
  development shell lacks. notify 8.2.1 (CC0-1.0, 6 crates) tells a change in 20 to 33 µs, where
  polling a file costs about 2 µs at each look: 208 ms at 100 000 files, twice a second.
- ***Hunna Wood*'s zone is laid out by a Blender script.** Its trees, rocks and ferns are scattered
  from a seed into one glTF file of 507 nodes, which a scene file of 56 lines places with six
  lanterns: moving a tree in the editor would override a node of a file that the next export from
  Blender fights.
- **CI's `check (windowed)` job took 23 minutes 31 seconds** at M6b.8b, past the owner's limit of
  20, and every windowed session of the editor adds to it.

Already decided, and not reopened: Play runs the game in a child process, in its own window,
linked by the development channel, started again on a save after its code changed, until Play takes
more than 3 seconds or what the channel shows of the game is not enough
([ADR-0110](0110-how-the-editor-runs-a-games-code.md)); a level is a composition of files, former
identities are written when an entity leaves its file, `EntityRef` is written as an absolute
identity and built with the inspector, cells are not identities, and the form of story layers is
fixed at Stage 6 ([ADR-0112](0112-levels-saves-and-worlds-for-open-worlds.md)); commands by
persistent identity, a deletion undone by `Restore`, the journal, the selection restored by undo,
the program on `slime-app`, one editor per journal ([ADR-0109](0109-stable-references-and-undo.md),
ADR-0134, ADR-0135); a retained interface, widgets as element kinds, a viewport as a view in its own
target, rfd 0.17 for dialogs, winit 0.31 once released, for drops on Wayland
(ADR-0132, [ADR-0148](0148-widgets.md)); drawing in the scene and over the image, picking by the GPU
and the CPU, views' settings and modes of debugging as reflected values
([ADR-0115](0115-what-the-gpu-layer-gives-the-editor.md),
[ADR-0127](0127-the-settings-of-a-view.md), [ADR-0129](0129-drawing-for-the-editor.md),
ADR-0130); shortcuts by a key's meaning, moves by its place, remapped by a `.keys` file
([ADR-0142](0142-input-and-focus.md)); the palette, notifications, menus, floating panels and F6
([ADR-0150](0150-layers.md), [ADR-0151](0151-rich-widgets.md),
[ADR-0153](0153-tab-and-f6-in-a-games-interface.md), ADR-0154); the cooker off the GPU
([ADR-0069](0069-the-asset-pipeline.md)); no source file makes a program panic
([ADR-0096](0096-the-files-the-engine-reads.md)).

The owner took sixteen decisions on 2026-10-27, one at a time, each as the synthesis recommended,
among the options that "Alternatives considered" lists.

## Decision

**The editor's shape, on which the four designs agreed**, and which the decisions below build on:

- One crate, `slime-editor`, holds the core, the window, the viewport, Play and the index of
  assets, as modules, until Stage 8's plugins show a boundary. The core stays without a GPU, tested
  without a window. The editor stays a program of `slime-app` (ADR-0134): the core is its
  simulation, its window a presentation; its run without a window stays, for CI's agents.
- Docking is an element kind of `slime-ui`, its layout reflected data that `slime-serialize`
  writes; the editor owns its panels and when their layout is saved.
- Agents never meet a dialog of the system: opening, saving and importing take a path, which a
  dialog only fills in for a person. A dialog runs on a thread of its own, the loop going on.
- The hierarchy is a list of the rows that show, which the editor flattens from the world's
  children, with the roles of a tree and rows that are selected. The inspector is built on the dev
  overlay's editors of reflected values (ADR-0152), whose builder of an entity's components becomes
  public; it adds and removes components, edits several entities at once, a mixed value shown as
  "—", marks an override with a bar and a bold label, with Revert and Apply for each field, and
  leaves driven fields read-only. What is hidden, locked or folded, the cameras and the steps of
  snapping are the user's, never the level's, and never undone: the editor hides nothing through
  `World::is_hidden`, a flag of the level that the frame builder reads. One history per level, not
  per file, which a panel shows.

**The window reads the core, and asks it through typed requests (D1).** Inside the editor's
program, the core and its window's presentation share a value, which the core writes and the
presentation reads: the selection, the history, the files not written, Play's state. The
presentation asks the core to act through typed requests, which one handler of the core answers;
the development channel's `Edit` requests become the same requests and go through the same handler.
A person and an agent act the same way, and a drag sends no text at each frame. This is an exception
to [ADR-0121](0121-a-game-as-a-simulation-and-a-presentation.md)'s rule that a presentation reads
the world alone, for the editor's program only: the editor's state is never saved, replicated nor
undone, so it is not the world's data, as ADR-0132 argued for the interface's elements. A game's
presentation keeps the rule.

**Every action of the editor is a named operation (D2).** One registry, in the core, holds every
*operation* of the editor: its name, its label, its shortcut and its parameters. Menus, shortcuts,
the palette, toolbars and agents run the same operation by its name; every drag, handle moved,
drop and panel moved has a path by the keys and an agent's path too. The operations that change the
level are steps; those that change the view, the camera, the layout, what is hidden or folded, are
not, and never enter the history. The development channel gains two requests, the operations
listed and one run by name with its arguments, version 10 of its protocol; the MCP server gains two
generic tools for them, beside those of ADR-0135, so that a new operation reaches agents with no new
tool. A check of CI finds every item of a menu and every entry of a `.keys` file in the registry.
The word *operation* keeps the world's `Command` apart.

**A gesture is one step, through an open step (D3).** While a person drags a handle, a slider or a
field's number, the core holds an *open step*: its commands are applied to the level at once, so
that the inspector, the children and the lights follow, but neither journaled nor in the history.
Escape undoes them and closes it; letting go makes them one step, journaled as any other. A crash
during a drag loses the drag alone. An agent's requests that come while a person holds a gesture
wait until it ends; the channel has no open step, an agent sending the final step. This amends
ADR-0134's history, where every change is a step as it is made.

**Frames on demand (D4).** `slime-app` runs a frame only when something asks for one, for a program
that chooses it, the editor's: an event of the window; a request of the development channel, whose
listener wakes the loop; a timer that the interface asks for, a caret, a transition, a
notification; a load or a cook that ends; a message of the game that the editor plays. The wake from
another thread is built on a channel and winit's own wake-up, not on a user event, which winit 0.31
removes. While frames are asked for, the scheduler paces them as ADR-0021 says; a game keeps its
scheduled frames. The interface and each viewport draw again only when what they show changed, a
viewport once more `RESIZE_DELAY` after its size last changed (ADR-0127); of several viewports, only
the one being navigated draws at the full rate. A check of CI runs an idle editor for 10 s and finds
that it ran no frame. In the stall measured above, a view drawn again costs 7 to 9 ms of CPU, and
84 ms on lavapipe: an idle editor that draws costs every machine, a laptop, a handheld, a game
beside it. Whether the program's run without a window waits the same way, rather than at
ADR-0135's pace, is milestone 1's to say.

**The viewport.**

- A new element kind, `Kind::Surface`, gives the program the pointer's actions and holds the
  pointer during a drag, where `slime-ui` takes every press on what it draws; while the right button
  flies, single-key shortcuts are suspended.
- Navigation as most editors have it: fly with the right button held and W, A, S, D, Q, E; orbit
  with Alt and the left button; pan with the middle button; F frames the selection. A `.keys`
  preset gives Blender's way beside it. The cursor is held, locked on Wayland and Windows; on X11,
  where winit refuses to lock it, confined, hidden and put back; where locking fails, a virtual
  machine or a remote desktop, the cursor stays free and the editor says so once. The raw motion
  comes from winit's device events.
- A click picks by the GPU, answered a frame or two later, and selects the outermost placement; a
  second click goes one level deeper; Alt and a click go through what lies behind, by the CPU's
  ray; a marquee selects what its area holds. Handles are hit by the CPU at once. No highlight
  follows the pointer in Stage 6c: a pick at every frame would draw the view again, at 7.5 to 8 ms
  in the stall; a highlight by the CPU's ray is weighed again later.
- The selection is outlined from a mask of what is selected, the scene's depth resolved, its edges
  drawn over the image, its hidden part dimmed; an entity without a mesh, a collider or bounds, is
  drawn as its transform's unit box when selected, which needs no game code. Handles are unlit
  instances over the image, with a depth of their own.
- Handles are the engine's own, mapped onto the world's uniform scale and its stretch, which no
  child inherits; snapping by increments and to an absolute grid; End drops the selection on what
  lies below, by the CPU's picker, without physics.
- Modes of debugging: base color and light alone, as branches of the shaders, and a wireframe of
  the selection alone. A wireframe of the whole scene waits until `PolygonMode::Line` is measured
  against lines along the edges.

**Creating.** Ctrl+D duplicates, the references inside the copy pointed at the copy, nested
overrides kept; copy and paste carry commands as text on the system's clipboard; the last operation
repeats; Ctrl+G groups; a *stamp mode* places a kit's variants at random under the cursor, with the
asset browser, and can be cut. Blender's modal G, R and S come later.

**Play hands the game what the editor holds (D5).** The editor starts the game's program as ADR-0110
says, through `slime_channel::launch`, with a token and a save of the editor's own, and hands it the
scenes that hold steps not written as private copies of their text, through an option of
development builds that the engine handles once for every game, rather than each game for itself:
the user's files are left alone, and Play neither writes nor cooks them. A prefab created since the
last cook must be cooked before it plays. While the game runs, each step that the editor makes is
sent to it as the same commands, over the channel, which a running game accepts since ADR-0134; a
command that the game refuses, as when its run changed or destroyed what it names, is shown, not
hidden. That the engine can give a run scenes' text is verified as the milestone starts:
`SceneLibrary::insert_file` takes a scene's text. Designers expect Play not to write their files,
and edits that restarted the run would defeat editing while playing. A value read in the running
game turned into a step of the level, "keep this value", comes with Play or later.

- Play, Pause, Step, Speed and Stop in a toolbar; a console of the game's log; a game whose code
  changed built again and started on a save of its run (ADR-0110), a mark on Play saying so. The
  build is read from cargo's messages in JSON, through `serde_json`, and the executable built is
  started itself, not `cargo run`; on Windows, without a console window. A running executable that
  Windows keeps locked may block the link: to check.
- The MCP server drives two programs, the editor and the game it plays, as ADR-0110's token handed
  over already implies.
- A game started with a token ends when its standard input closes, so that an editor, an MCP server
  or a test that crashes leaves no game behind; the standard library suffices.
- **Updating a running world in place, keeping the handles that a game holds, waits for Stage 8
  (D12)**, with the scripts' reload. [ADR-0089](0089-scenes-and-prefabs.md) placed it with Stage 6's
  editor; with the editor's edits sent as commands, no reload happens while the editor plays, and
  handles are kept anyway.

**A level names its parts (D6).** A level's file names its *parts* in a table of their own,
`[parts.<id>]`: each the scene file of a part, which the level places and the editor edits in place
and writes to its own file. Any other placement places a prefab, which the editor also opens in
place, *open in context*, its changes written to the prefab's file. The table leaves room for the
condition that Stage 9b's layers put on a part (ADR-0112). The layout of scene files changes once,
for the parts and for the absolute identities with which `EntityRef` is written (ADR-0112): the
same change writes former identities, and says whether a file records the parents and the order of
a placement's entities, which the known debt asks; their form is decided by the milestone that
builds the layout, in its ADR, which amends [ADR-0091](0091-scene-documents-and-placements.md)
again.

**The editor opens a level that the world would refuse for what it does not know (D7).** A
component or a field that the program does not know, a game's type renamed, does not keep the
editor from opening a level: the editor keeps its text opaque, shows it as "unknown (kept)", and
writes it back byte for byte, splicing it into what the world writes as `keep_comments` splices
comments. The world stays strict, and a game still refuses the scene (ADR-0089). That the text can
be spliced back is verified as the milestone starts.

***Hunna Wood*'s zone moves onto a kit of prefabs (D8)**, which proves the exit on the game's real
level. Blender exports pieces of terrain and one prefab per variant of tree, rock and prop, each
with its collider; an agent converts the scatter into placements once, through the editor's own
operations, some 250 of them, which test the editor and principle 10 at scale; the owner's exit
check erases the campfire's clearing and builds it again by hand. The save fixture is made again,
which [ADR-0100](0100-updates-of-shipped-games.md) allows before the game's first release, and the
zone's tests are written again; the game's README says it when it is done.

**Docking, layouts and one window (D9).**

- *The model*: splits of any number of children, each with its share of the space, and stacks of
  tabs, normalized after each edit, so that no split is empty or holds one child; a panel closed
  keeps its place, to come back there; the default layout carries a version, so that a newer default
  replaces a layout saved against an older one. Resizing a border changes the shares of the two
  panels beside it and nothing else, and saved layouts stay stable.
- *In steps*: regions with tabs, splits that the user resizes, "Move to" and floating panels come
  with the hierarchy; panels dragged to dock come last, and move to Stage 11 if the stage runs long,
  with an amendment of ADR-0131 and the roadmap, the regions serving meanwhile.
- *Where*: per user and per project, outside the repository, in the user's state directory, XDG's
  on Linux and `%LOCALAPPDATA%` on Windows, where XDG's specification puts such state; a game's
  editor package gives the project's default. A layout is read leniently; one that cannot be read
  is set aside, the default used and the user told; "Reset layout" always works. Named layouts wait
  until someone asks.
- *One window of the system* in Stage 6c, with its floating panels inside it
  (ADR-0150, ADR-0154): no panel in a window of its own, which Wayland cannot place. One window per
  process: each `Gpu` gets an identity that its objects carry, so that an object used with another
  `Gpu` panics rather than acting on another object, which settles G34.

**The tools watch the sources through the system (D10).** The editor's asset browser and
`slime-cook watch` learn of a source's change through notify 8.2.1, scan again when the system's
queue overflows, and fall back on polling where its notifications fail. Games keep polling their
manifest and shaders, one file or a few ([ADR-0076](0076-hot-reload-of-cooked-data.md),
[ADR-0077](0077-hot-reload-of-shaders.md)). [ADR-0072](0072-stage-4b-live-assets.md) rejected
notify for games, to gain half a second; the tools' reason is the cost of polling as a project
grows. It is adopted at the asset browser's milestone, once polling is measured on Windows.

**The assets.** The browser's index is the cooker's manifest and a scan of the sources. Thumbnails:
a texture shows a small level of its cooked KTX2; a mesh or a prefab is drawn offscreen by the
editor, a few a frame, only for the tiles that show, read back without waiting, encoded with `png`
at `Compression::Fast`, and cached in the user's cache directory under the cooked file's name, a
hash of all that made it, so that a thumbnail never goes stale and never enters the repository; the
GPU keeps those used last. The cooker cannot draw them (ADR-0069). Cooking from the editor runs in
its process, on the background priority, once two cooks in one process no longer share their
temporary files (G49). Files dropped on the window, on Windows and X11, are imported only; placing
them where they fall, and drops on Wayland, wait for winit 0.31, whose move gets a milestone and an
ADR of its own when it is released, and does not hold 0.7 back. `slime-app` gains the user's state
and cache directories beside the data directory, without a crate.

**Quitting asks nothing (D11).** The editor quits at once with steps not written; its journal
brings them back at the next start (ADR-0135); the window's title and the status bar show how many
steps are not written, all along. Neither a person nor an agent meets a dialog, and nothing is lost.

**First run, and names.** The editor opens its program's level at once, cooking the examples if they
are missing, with the recent levels in the File menu and one notification as a hint; no start page
nor manager of projects before Stage 11. The world gains a tick for its entities' names apart from
its structure, which the hierarchy follows, and which also spares the dev overlay's world panel its
list made again at each rename.

**The milestones, in this order (D13)**, each after what it needs, what a designer uses first, Play
early enough to check ADR-0110's conditions, and the file format before the kit, so that the exit's
rehearsal runs on the final format:

0. *Before the stage*: the channel's messages written at once, Nagle's algorithm off at both
   ends; CI's windowed checks split into a third job (D16); a game started with a token ending when
   its standard input closes; how to make Weston present on a GPU (`--renderer=gl`) written
   down.
1. *The editor's window*: D1, D2, D4, D11; the menu bar, the status bar, dialogs, the raw motion
   and the cursor held; the dev overlay's world panel docked as the hierarchy and the inspector
   until milestone 5.
2. *The viewport*: navigation, the grid, icons, picking, the outline, modes of debugging, a second
   viewport.
3. *Move and place*: D3, the handles, snapping, creating, placing prefabs, the history panel.
4. *Play it*: D5.
5. *Find and change*: the hierarchy and docking's regions, then the inspector, likely in two.
6. *Levels of several files, and links*: D6, D7, then `EntityRef` and its picker, likely in two.
7. *Build from a kit*: the asset browser, D10, D8.
8. *Panels dragged to dock*, which may move to Stage 11.
9. *The Windows check, the documents, and the release of 0.7.*

The owner tries the editor twice between them, by hand, on Linux and in the Windows virtual machine:
*Checkpoint A* after Play, an hour of flying, selecting, rearranging and playing the zone, whose
frictions adjust the plan and decide what to cut if the stage runs long; *Checkpoint B* after the
kit, the exit's rehearsal, the campfire's clearing built again by hand, played, written, opened
again, undone and redone through it all. Any milestone may be cut in two at its start, as Stage
6b's were. The exit needs at least milestones 1 to 3, 5 and 6, and placing from a list of prefabs;
it can do without docking by drag, thumbnails, stamp mode, several viewports, a view of the running
game, "keep this value", the notify watch and modal keys. Play (ADR-0110, ADR-0131) and the
file format (ADR-0112) belong to the stage by decision, not by the exit.

**One release, 0.7 (D14)**, as ADR-0131 says. The exit is met at Checkpoint B, after which only
docking by drag and the release remain; a second release would hold almost nothing.

**CI's windowed checks in a third job (D16).** `scripts/check.sh` gains a third part, which CI runs
in a third job beside the two of [ADR-0123](0123-the-linux-checks-in-two-jobs.md), each within
the owner's limit of 20 minutes, which stays; milestone 0 chooses which runs each part holds.

**Dependencies**, each added at the milestone that first uses it and recorded in
[`docs/dependencies.md`](../dependencies.md) then:

| Crate | License | For | Milestone |
|---|---|---|---|
| rfd 0.17 | MIT | Dialogs to open and save, already approved by ADR-0132; D-Bus's library added to the development shell's | The editor's window |
| serde_json, already in the lock for `rmcp` | MIT OR Apache-2.0 | A new edge, into `slime-editor`: cargo's messages read when the game is built again | Play it |
| notify 8.2.1, 6 crates | CC0-1.0 | The tools' watch of the sources | Build from a kit |

**The roadmap.** Stage 6c lists what this decision builds; Stage 8 gains the world updated in place;
Stage 11's line "saved layouts, multi-window, command palette" becomes panels in windows of their
own, since the palette came with 0.6 ([ADR-0151](0151-rich-widgets.md)) and layouts come with
Stage 6c.

## Alternatives considered

How the window reaches the core (D1):

- **`Context::edit` in `slime-app`, answered by `Simulation::edit` as the channel's requests are,
  with a snapshot that the simulation publishes**: the window takes the agents' very path, but the
  commands go as text at each frame of a drag, the answers come a frame late, and the engine gains
  an API.
- **The editor's state as components of an entity of the edited world**: no rule bent, agents read
  it as the world; but tooling data in the world, which writing and saves must skip, and each
  selection moving the world's ticks.
- **A generic message API in `slime-app`**, a presentation posting to its simulation: games' menus
  might use it at Stage 7, but an abstraction with one user, which still reads nothing.

Operations (D2):

- **One `Edit` request and one MCP tool per operation**: typed, and listed by MCP clients, but the
  server grows with each operation, and drifts from the editor.
- **The interface's command action alone**: it exists, but carries no arguments.

A gesture (D3):

- **A preview drawn by the window alone**, applied at the release: the inspector, the children and
  the lights stale during the drag.
- **A step at each frame, merged afterwards**: the journal flooded.
- **An open step on the channel too**, three more requests: agents gesture as people do, but more
  protocol, for an agent that can send the final step.

When a frame runs (D4):

- **A rate of 10 frames a second while idle**, with frames on change: simpler, but the channel's
  answers up to 100 ms late, a little processor forever, and a redraw that never stops unnoticed.
- **Every frame**: nothing stale, but the processor and the GPU busy all along, worst in the stall.

Play (D5):

- **Play writes the level's files, cooks them, and the game runs with `--watch`**, edits going
  write, cook, reload: the least new code and the file the one truth; but Play writes files under
  version control that the user did not ask to write, edits arrive after about a second with the
  run carried and its handles changed, and *Hunna Wood* starts its run again when its layout
  changes, which defeats editing while playing.
- **The level sent over the channel after the start**: no files, but slower from a cold start, and a
  new path of the protocol.
- **Updating the world in place with Play (D12)**: handles kept at a reload, but about one
  milestone more, for a reload that commands avoid.

Parts (D6):

- **Open in context alone**, a scene placed once taken for a part: no change of the format, but
  "placed once" is a guess, and a scene file used as a prefab becomes ambiguous.

A level that does not read cleanly (D7):

- **Refused, as today**: strict, but a renamed type locks the level out of the editor.
- **Opened, what is unknown dropped and reported**: data lost without a choice, as O3DE's slices
  lost it.

The level that proves the exit (D8):

- **The zone as it is, edited through overrides**: no content work, but not built in the editor, and
  Blender and the editor fight.
- **A new small level built from a kit**: an honest proof, but the game plays one zone, whose ID it
  holds, and the real zone stays Blender's.

Docking (D9):

- **Binary splits**: simpler, but resizing one border moves several panels, and saved layouts
  change shape as panels move. **Fixed slots**, as Godot's docks: less to build, but the user
  arranges only what the slots allow.
- **Docking by drag first**, as the provisional order had it: built before any panel uses it,
  against the rule of a second use that ADR-0116 and ADR-0131 applied. **No docking in Stage 6c**:
  against ADR-0131.
- **Layouts per user only**, or **in the repository**: a project could not differ, or each user's
  arrangement would churn the project's history.
- **Panels in windows of their own**: Wayland cannot place them, and a second window on one GPU is
  what G34 lacks.

Watching (D10):

- **Polling everywhere**: no dependency, but nearly half a core of the processor at 100 000 files,
  while the editor watches.
- **notify everywhere, games included**: one way, but games watch one file or a few.

Quitting (D11): **asking to save, not to save or to cancel**: familiar, but a dialog for agents, and
the journal already keeps every step.

The order and the releases (D13, D14):

- **The provisional order, docking first**: docking without a panel.
- **Play second**, to check ADR-0110 early: the measurements already put Play far within 3 s.
- **The file format before Play and the asset browser**, the dearest to change: the order keeps that
  part, before the kit, but a designer would wait longest for what they use first.
- **Two releases, 0.7 at the exit and 0.8 after**: every later version moved again, as ADR-0131 did
  once, for a second release of almost nothing.

CI (D16): **a new limit of 30 minutes**: no work, but the job's own limit leaves no margin, and each
push waits longer, as ADR-0123 found.

The smaller choices:

- **A highlight under the pointer by a GPU pick at each frame**: a redraw of the view at each frame.
- **The middle button orbiting, as Blender's, with both mappings on at once**: one default is easier
  to teach.
- **A start page with recent levels**: a screen before the work, where the program already knows
  its level.
- **transform-gizmo 0.11** for the handles: refused by `deny.toml`'s licenses, and 26 to 28 crates.
- **The `directories` or `dirs` crates** for the state and cache directories: about 40 lines do it.
- **The `image` crate** for thumbnails: `png` is already built.
- **process-wrap**, or Linux's `PR_SET_PDEATHSIG`, to end an orphaned game: a dependency, or one
  system.
- **Thumbnails beside the sources**, or drawn by the cooker: churn in commits, or a GPU in the
  cooker, which ADR-0069 keeps off it.

## Consequences

- The editor's program is the one place where a presentation reads more than the world; if a render
  thread ever takes extraction apart (Stage 7, [ADR-0111](0111-the-frame-contract.md)), the value it
  shares with its core must be made shareable across threads.
- Every operation of the editor reaches agents through two tools; the development channel's protocol
  goes to version 10, and the MCP server drives two programs at once.
- `slime-app`'s loop can wait without a frame: an idle editor costs nothing, and every wake-up from
  another thread must be right, which CI's idle check guards.
- Play never writes the user's files; the engine gains an option of development builds that gives a
  run the text of scenes; a step made while playing can be refused by the game, and says so.
- The layout of scene files changes once, at milestone 6; the examples' yard and *Hunna Wood*'s
  zone are written in it.
- *Hunna Wood*'s zone becomes the editor's: its save fixture and its tests are made again, and its
  Blender script exports a kit rather than a scattered land.
- `slime-ui` gains docking, the editor's layouts live outside the repository, and Stage 6c has one
  window of the system per process.
- rfd, notify and a new edge of `serde_json` enter; CI uses one more runner.
- ADR-0041's stall decides how the editor feels on machines without resizable BAR, the owner's
  among them: the viewport's milestone measures `PASSES_PER_SUBMISSION` and the bloom there, and a
  fix of the renderer stays outside the stage unless the owner brings it in.
- If docking by drag moves to Stage 11, ADR-0131 and the roadmap are amended then.
- [`docs/ROADMAP.md`](../ROADMAP.md) shows Stage 6c as decided, Stage 8's update in place and Stage
  11's corrected line; [`docs/PLAN.md`](../PLAN.md) details the milestones, the checkpoints and the
  debts that each settles.
