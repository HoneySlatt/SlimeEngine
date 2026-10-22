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

- Stage 6a — What the editor stands on: done, and released as 0.5 on 2026-10-22 with the audit pass
  and the foundations pass ([ADR-0116](adr/0116-stage-6-in-two-halves.md) to
  [ADR-0130](adr/0130-picking.md)). *Hunna Wood* runs on what the editor stands on, on Linux and on
  Windows: frustum culling and instancing, the engine's task pool and the copies to the GPU within
  a budget, the world's ticks, a frame that owns what it draws, a game as a simulation and a
  presentation, with a game time that pauses and steps, the development channel and the MCP server
  through which an agent drives it, saves made of differences per scene, views with settings of
  their own, asset requests, drawing for the editor and picking. The audit pass and the foundations
  pass before it fixed what the audit of 0.4 found, and decided as ADRs what the editor freezes
  ([ADR-0096](adr/0096-the-files-the-engine-reads.md) to
  [ADR-0115](adr/0115-what-the-gpu-layer-gives-the-editor.md)); the games that the engine aims at
  are described in [ADR-0098](adr/0098-the-games-the-engine-aims-at.md), and AI agents drive the
  engine through its MCP server ([ADR-0099](adr/0099-ai-agents-drive-the-engine.md)). See
  [Completed stages](#completed-stages).
- Stage 6b — The engine's interface: planned on 2026-10-22 and validated by the owner
  ([ADR-0131](adr/0131-stage-6b-and-stage-6c.md)), to be released as 0.6 at its close. A study of
  the UI framework's design, made by four agents from four starting points and measured on the
  candidate crates, led to the owner's decisions on the framework
  ([ADR-0132](adr/0132-the-engines-ui-framework.md)) and on the fonts that the engine ships
  ([ADR-0133](adr/0133-font-files-under-the-open-font-license.md)). Stage 6 is now in three parts:
  Stage 6c, the editor, is planned in detail at Stage 6b's close, and released as 0.7. M6b.1 was
  cut in two at its start ([ADR-0134](adr/0134-the-editors-core.md)): M6b.1a, the commands and
  undo, is done; M6b.1b, the editor driven by an agent
  ([ADR-0135](adr/0135-the-editors-program.md)), is built and passes `scripts/check.sh`, and waits
  for CI. Next: M6b.2.

## Status

| Milestone | Goal | Status |
|---|---|---|
| [M6b.1a](#m6b1a--the-commands-and-undo) | The world's commands name entities for good and undo themselves; the editor's core makes them into steps, undone, redone and journaled, and writes its level back with its comments | Done |
| [M6b.1b](#m6b1b--the-editor-driven-by-an-agent) | An agent drives the editor's core through the MCP server, on a level of the examples and on *Hunna Wood*'s zone | Built; CI to run |
| [M6b.2](#m6b2--text-in-several-writing-systems) | The engine lays text out in every writing system, without a GPU | Planned |
| [M6b.3](#m6b3--drawing-the-interface) | The renderer draws an interface's boxes, text and images, crisp at every scale | Planned |
| [M6b.4](#m6b4--elements-styles-and-layout) | An interface is a tree of elements, styled by themes read from files, laid out by flexbox and grid, and built again only where what it shows changed | Planned |
| [M6b.5](#m6b5--input-focus-and-what-agents-see) | A person and an agent use the interface through the same actions, and an agent reads what it shows | Planned |
| [M6b.6](#m6b6--text-edited-and-what-the-platform-lacks) | Text is typed, selected, copied and composed in every writing system, and screen readers read the interface | Planned |
| [M6b.7](#m6b7--widgets-and-layers) | The framework has the widgets that the dev overlay and the editor are made of | Planned |
| [M6b.8](#m6b8--the-dev-overlay-on-the-engines-interface) | The dev overlay runs on the engine's interface, and egui leaves the engine | Planned |
| [M6b.9](#m6b9--windows-check-documentation-release-06) | Stage 6b is done, checked on Windows, and released as 0.6 | Planned |

Stages 0, 1, 2, 3a, 3b, 3c, 3d, 4a, 4b, 5a, 5b and 6a are done, and so are the audit pass and the
foundations pass; see [Completed stages](#completed-stages).

## Stage 6b — The engine's interface

Exit criterion (see [`ROADMAP.md`](ROADMAP.md#stage-6b--the-engines-interface--06)): the dev overlay
of *Hunna Wood* and of the `world` example runs on the engine's UI framework, on Linux and on
Windows, its theme read from a file, and the framework draws text in several writing systems; an
agent reads and drives the overlay's panels through the engine's MCP server, and a screen reader
reads those of the `world` example; the list of a world of 100 000 entities stays within the budget
that M6b.4 sets for it; egui is gone. The editor's core changes a level through commands, undone and
redone, written back and opened again, driven by an agent.

This is the second part of Stage 6 ([ADR-0131](adr/0131-stage-6b-and-stage-6c.md)): the engine's
UI framework, which [ADR-0098](adr/0098-the-games-the-engine-aims-at.md) makes the engine's own,
for its editor and for the UI of games, proven by the dev overlay that games ship; and the editor's
core, without its interface. Where the engine stands at the start:

- **The world takes one command.** `Command::SetProperty` names its entity by its persistent
  identity and is applied at the start of a frame
  ([ADR-0121](adr/0121-a-game-as-a-simulation-and-a-presentation.md)); nothing creates an entity
  under a given identity nested in a scene's, nor puts a child at a given rank among its siblings
  (`World::set_parent` puts it last), `World::despawn` forgets an identity, and nothing undoes
  ([ADR-0109](adr/0109-stable-references-and-undo.md)).
- **The only interface is the dev overlay, on egui**
  ([ADR-0031](adr/0031-dev-overlay-with-egui.md)): immediate mode, its world panel laying out every
  row (G46), its number fields writing back the rounding they show (G33), its renderer, egui-wgpu,
  the one library outside `slime-gfx` that reaches wgpu, through `slime-gfx`'s `wgpu` feature, its
  fonts shipped by an exception of `deny.toml`
  ([ADR-0105](adr/0105-egui-fonts-ship-with-the-dev-overlay.md)).
- **The platform reads keys, the mouse and the scale factor of one window**: no input methods,
  clipboard, cursor shapes, files dropped, dialogs, raw motion of the mouse nor screen readers
  ([ADR-0011](adr/0011-windowing-with-winit.md)).
- **The renderer draws views**, and lines and unlit instances into them
  ([ADR-0129](adr/0129-drawing-for-the-editor.md)); nothing draws rounded rectangles, glyphs or
  images in two dimensions. `slime-gfx` has no texture of one channel, writes no part of a texture,
  and its scissor is its viewport.
- **Agents read a running game's world, captures, reports and picks**
  ([ADR-0122](adr/0122-the-development-channel.md)), nothing of an interface.

Out of scope: Stage 6c's work, docking, menu bars, dialogs to open and save files, files dropped,
drag and drop between panels, the editor's window and viewport, play in the editor, the hierarchy,
the inspector beyond the overlay's value editors, `EntityRef`, gizmos, levels of several files and
the asset browser; several windows of the system (G34); Stage 7's UI for games, animation and
transitions, layers with masks, effects on text through distance fields, UI documents and their
bindings, UI on surfaces in the world, focus driven by gamepads; translation tables (Stage 9b).

Decisions taken when the stage was planned, by the owner on 2026-10-22: Stage 6b split again, with
its order and its releases, in [ADR-0131](adr/0131-stage-6b-and-stage-6c.md); the framework's
design, its crates and its dependencies, in [ADR-0132](adr/0132-the-engines-ui-framework.md); the
fonts that the engine ships, in [ADR-0133](adr/0133-font-files-under-the-open-font-license.md).
Each milestone that designs a part records its design in a new ADR, which amends ADR-0132 where it
changes it; each dependency is added at the milestone that first uses it, its version and what it
adds to CI's build checked then.

Risks:

- **The size of the framework.** The study counted 18 000 to 25 000 lines of Rust for it, where
  each of Stage 6a's milestones added 1 000 to 4 000. A milestone that outgrows itself is split at
  its start, as M6a.7, M6a.8 and M6a.10 were.
- **Dependencies that change fast.** parley, taffy and AccessKit published a version that breaks
  their API every one or two months in 2026. Their types stay inside the engine's crates, their
  versions are pinned, and each is upgraded at most once a milestone.
- **The cost of the checks.** CI's `windows` job took 19 minutes 55 seconds on the release commit of
  0.5, and its `check (windowed)` job 20 minutes 29 seconds at M6a.9, at the owner's limit of 20
  ([ADR-0104](adr/0104-the-build-ci-and-measurements.md)); the framework's crates bring about 40
  that the engine does not build today, ICU4X's most of them, and AccessKit's adapter 46 more on
  Linux. Each milestone that adds a crate records how long CI's jobs take, and a job past the limit
  is split, as ADR-0081 and [ADR-0123](adr/0123-the-linux-checks-in-two-jobs.md) split them; about
  20 crates leave with egui at M6b.8.
- **What only a person checks.** The look of text, input methods on Wayland and on Windows, the
  clipboard on Wayland's compositors and screen readers cannot be checked in a headless Weston: each
  milestone gives its checks by hand to [the list](#checks-by-hand-waiting-for-the-owner), but for
  the choices that it needs from the owner to go on, the blending and the font of M6b.3, made during
  it.
- **Wayland.** arboard reaches Wayland's clipboard through the data-control protocol, which niri and
  Hyprland offer and not every compositor does; winit 0.30.13 drops no file on Wayland (Stage 6c).
- **The overlay rewritten.** Games and their checks by hand use it: M6b.8 writes a checklist of
  what it does today, and egui leaves only once the new one does all of it.
- **One GPU.** Budgets are checked on the owner's RX 6900 XT only ([Known debt](#known-debt)).

### M6b.1a — The commands and undo

- **Goal.** The world's commands name entities for good and give back what undoes them; the
  editor's core makes them into steps, undone, redone and journaled, and writes its level back with
  its comments.
- **Crates.** `slime-world`, `slime-serialize`, `slime-cook`, a new `slime-editor`; `slime-app`;
  the benchmarks.
- **Delivers.** As [ADR-0109](adr/0109-stable-references-and-undo.md) decides, and
  [ADR-0134](adr/0134-the-editors-core.md) shapes: the world creates an entity under a given
  identity nested in a scene's, refused if the identity is taken, places a prefab so, and puts a
  child at a given rank among its siblings. The world's commands are the editor's: create, place a
  prefab, delete, reparent, set a property, attach and detach a component, and restore what a
  deletion recorded, its subtree, the components as reflected values, the ranks and the references
  that named it; each gives back the command that undoes it, and is written and read as text, its
  entities by their identities. `slime-editor`, a library without an interface: a level opened from
  its scene file, changed through steps, made whole or not at all, undone and redone within a bound
  of memory, the selection by identities, restored by undo and redo; the journal, as text, from
  which an editor that crashed is recovered; the level written back, whole or not at all, with its
  comments, what the editor created named at once as the file names it. The ADR of the design; the
  rustdoc, `docs/ARCHITECTURE.md`, the glossary, `CHANGELOG.md`.
- **Done when.** Tests: each command, done, undone and redone, leaves the level that the file writes
  as it was, identities, ranks and references included; a subtree deleted, which other entities
  named, comes back named by them; a step refused half-way leaves the level as it was; an editor
  recovered from its journal holds the level, the history and the selection of the one that
  crashed, a writing of the level and undoes across it included;
  a level opened, changed, written back and opened again holds the changes and its comments, and
  what the editor created there has the identity that the file gives it; a benchmark of a thousand
  entities deleted then restored (the audit estimated 10 to 15 ms); CI is green.
- **How to test.** `cargo test -p slime-world -p slime-serialize -p slime-editor`;
  `cargo run --release -p slime-benches -- world.undo`.
- **Outcome.** Done on 2026-10-22. [ADR-0134](adr/0134-the-editors-core.md) records the owner's
  eight decisions of 2026-10-22, among which M6b.1 cut in two, and amends ADR-0089, ADR-0092,
  ADR-0109, ADR-0110, ADR-0121, ADR-0122 and ADR-0131. `slime_world::Command` gains `Create`,
  `Place`, `Delete`, `Reparent`, `Insert`, `Remove` and `Restore`, and `World::apply` returns the
  command that undoes the one it applied; `Delete` records its subtree in `Deleted`, its values
  referring to entities by identity, which `Restore` creates again. `World::spawn_as`,
  `World::place_as` and `World::new_id` create under an identity of a scene; `World::set_parent_at`
  and `World::rank` place a child among its siblings. `Command::write` and `Command::read` give a
  command's text; `slime_world::Identities` names entities by identity for it and for the
  development channel, which used a copy of its own. `slime_serialize::keep_comments` puts the
  comments of a document written by hand back into the one written again; `slime_cook::write`, which
  writes a file whole or not at all, is public. `slime_editor::Editor` opens a level, makes steps of
  commands given as values or as text, refusing those that would change what is not under the
  level's root, undoes and redoes them (`HISTORY_BUDGET`), selects, keeps a journal, from which
  `Editor::recover` recovers an editor that crashed, and writes its level back. The examples' yard
  written back by the editor, unchanged, is the file written by hand, byte for byte, which a probe
  checked and M6b.1b's tests check. The benchmark `world.undo.1k` deletes and restores a subtree of
  1 000 entities in a level of 100 000 several times faster than the audit estimated
  ([`docs/measurements.md`](measurements.md)). A read-only review found twelve defects, among which
  a recovery that rebuilt another level when an undo crossed a writing, and entities left out of the
  level without a word: each is fixed, and tested. The tests: in `slime-world`'s
  `tests/commands.rs`, `each_command_done_undone_and_redone_leaves_the_file_as_it_was`,
  `a_deleted_subtree_comes_back_named_by_what_named_it`,
  `a_restoration_is_refused_while_an_identity_is_taken_and_changes_nothing`,
  `a_command_refused_changes_nothing`, `commands_written_as_text_are_read_back_the_same`,
  `text_that_is_not_a_command_is_refused_with_the_reason`,
  `what_the_world_names_is_free_and_written_as_the_file_names_it` and
  `a_prefab_placed_is_written_as_a_file_places_it`; in `hierarchy.rs`,
  `a_child_put_at_a_rank_goes_there_and_back_and_a_rank_past_the_last_puts_it_last`; in
  `slime-serialize`'s `comments.rs`, five tests, and its documents damaged, their comments kept; in
  `slime-editor`'s `tests/editor.rs`,
  `a_level_changed_written_and_opened_again_holds_the_changes_and_its_comments`,
  `steps_undo_and_redo_whole_and_select_again_what_they_selected`,
  `a_step_refused_half_way_leaves_the_level_as_it_was`,
  `a_command_that_would_change_what_is_not_under_the_level_s_root_is_refused`,
  `a_recovered_editor_holds_the_level_the_history_and_the_selection_of_the_one_that_crashed` and
  `the_history_forgets_the_steps_undone_then_the_oldest_done_past_its_budget_but_the_last`.

### M6b.1b — The editor driven by an agent

- **Goal.** An agent drives the editor's core through the MCP server, on a level of the examples
  and on *Hunna Wood*'s zone.
- **Crates.** `slime-editor`, `slime-app`, `slime-channel`, `slime-mcp`; the examples;
  `games/hunnawood` and a new `games/hunnawood-editor`.
- **Delivers.** As [ADR-0134](adr/0134-the-editors-core.md) decides: the editor's program, an
  application of `slime-app` whose simulation is the editor's core, without a window, its frames
  paced; the editor's requests reaching it through methods of `Simulation`, and the commands that
  the channel or the dev overlay send to it made steps of its history; version 7 of the
  protocol, a step of commands as text, undo, redo, the history, the journal, the selection read
  and set, and the level written back; the MCP server's tools for each. Two programs: the example
  `editor`, on the examples' yard, and `games/hunnawood-editor`, *Hunna Wood*'s editor package, on
  its zone, the game making public what registers its types. The journal kept in the editor's
  directory of the user's data directory, unless `--journal` names another file; what a program
  does with the journal that a crash left is decided at the milestone's start.
- **Done when.** Tests: an agent's session through the MCP server opens the yard, creates, places,
  changes, deletes, undoes and redoes, reads the history and the journal, writes the level back
  and opens it again; the yard written back unchanged is the file written by hand, byte for byte;
  *Hunna Wood*'s zone written back keeps its comments; the commands that the channel sends to the
  editor undo; CI is green, its jobs' times recorded.
- **How to test.** `cargo test -p slime-editor -p slime-app -p slime-mcp`; the MCP server's session
  test; `cargo run --example editor`; `cargo run -p hunnawood-editor`.
- **Outcome.** Built on 2026-10-23; `scripts/check.sh` passes, and CI runs once the milestone is
  pushed, its jobs' times recorded here then. [ADR-0135](adr/0135-the-editors-program.md) records
  the owner's decision of 2026-10-23, that a program recovers by itself the journal that a crash,
  or a stop without writing, leaves, and the design that the owner validated the same day; it
  amends ADR-0121, ADR-0122 and ADR-0134. `slime_editor::run` runs a `Program` on `slime-app`
  without a window, its frames paced (`AppConfig::pace_headless`); its journal is
  `slime-editor/<the level's asset ID>.journal` in the user's data directory
  (`slime_app::data_directory`), or one of its own for a copy of the level, held by one editor at a
  time (`slime_app::HeldFile`); a journal that cannot be read, played again or written stops the
  program, and `--discard` sets it aside (`slime_app::set_aside`). *Hunna Wood* gave itself the
  data directory, the lock and the setting aside before. `Simulation::apply` takes the commands that
  the host receives, which the editor makes steps of, and `Simulation::edit` the editor's requests,
  version 7 of the protocol (`Request::Edit`); the MCP server gains `edit`, `undo`, `redo`,
  `select`, `editor_history`, `editor_journal` and `write_level`. The example `editor` opens the
  examples' yard, and `games/hunnawood-editor` the game's zone, through
  `hunnawood::rules::register`, now public. Run by hand, *Hunna Wood*'s editor opens the zone and
  paces its frames at 60 a second. The zone written back keeps every comment above what it
  describes; its formatting becomes the engine's writer's, rotations whole as ADR-0092 writes them.
  A read-only review found nine defects, among which a recovery that failed half-way and emptied the
  journal that it recovered, copies of a level that shared one journal, and a level written over
  what another editor wrote, as well as gaps in the tests and the documents: each is fixed, and
  each defect tested. The tests: in `slime-mcp`, `tests/edit.rs`, an agent's session through the
  server, which writes the yard back unchanged, byte for byte, makes a step that creates, places,
  changes and deletes, changes a value with `set_property`, which becomes a step, undoes it and
  finds the value as it was, redoes, reads the history and the journal, writes the level, which
  holds the changes, stops the editor and finds again, as it starts, the step done after the
  writing and a step not written, then, the level written, opens it from its file; and
  `tests/edit_zone.rs`, a session on a copy of *Hunna Wood*'s zone, which changes one of the game's
  components, creates an entity that holds another, undoes, redoes and writes the zone; in
  `games/hunnawood-editor`'s `tests/zone.rs`,
  `the_zone_written_back_keeps_its_comments_and_opens_again_the_same`, with the cooked zone; in
  `slime-app`'s `tests/lifecycle.rs`,
  `a_paced_headless_run_waits_for_each_frame_and_still_steps_one_fixed_step_each` and
  `a_simulation_applies_the_commands_sent_to_it_its_own_way`, in `data.rs`,
  `one_program_at_a_time_holds_a_file` and `files_set_aside_keep_names_of_their_own`, moved from
  the game, and in `game.rs`,
  `a_simulation_that_is_not_an_editor_refuses_what_a_tool_asks_of_an_editor`; in `slime-channel`,
  the editor's requests and replies in `every_kind_of_message_comes_back_from_its_encoding`; in
  `slime-editor`'s `program.rs`,
  `options_name_another_level_assets_and_journal_or_discard_the_journal`,
  `a_session_recovers_what_its_journal_holds_that_the_file_does_not_and_only_that`,
  `a_journal_that_cannot_be_played_again_stays_as_it_was_and_discard_sets_it_aside`,
  `a_journal_that_is_not_text_stops_the_start_and_discard_sets_it_aside`,
  `a_journal_whose_last_entry_is_torn_stops_the_start_and_stays_as_it_was`,
  `one_editor_at_a_time_holds_a_journal_and_another_level_s_is_refused` and
  `copies_of_a_level_take_journals_of_their_own`, and in `journal.rs`,
  `a_journal_holds_steps_unwritten_after_its_last_writing_or_its_start`.

### M6b.2 — Text in several writing systems

- **Goal.** The engine lays text out in every writing system, without a GPU.
- **Crates.** A new `slime-ui`, its text module; fonts in the repository.
- **Delivers.** As [ADR-0132](adr/0132-the-engines-ui-framework.md) decides: fonts loaded from bytes
  and named in a font's stack; the system's fonts as a fallback where a program asks for it,
  fontconfig loaded at run time; parley wrapped: shaping, bidirectional order, line breaking with
  its `complex-scripts` feature, alignment, an ellipsis where a line is cut, the glyph under a
  point, carets and selections; glyph runs whose key holds the font, the glyph, the size, the
  position between pixels and the representation; glyphs rasterized by swash, into coverage or
  color; shaped text kept in a cache within a budget. Test fonts, small parts of Noto fonts with
  their licenses ([ADR-0133](adr/0133-font-files-under-the-open-font-license.md)), each fetched with
  the owner's agreement. The dependencies parley, fontique and swash. Benchmarks of shaping and
  rasterizing. The ADR of the design.
- **Done when.** Tests without a GPU: Arabic joined, and laid out right to left with Latin words and
  numbers in it; Hebrew; Devanagari's clusters; Thai broken between its words; Japanese and Chinese
  broken between characters; an emoji sequence as one glyph; no glyph missing; an ellipsis; carets
  and the glyph under a point in text of both directions. The benchmarks, on the owner's machine,
  stay within 20 % of the study's figures that [ADR-0132](adr/0132-the-engines-ui-framework.md)
  records, or the milestone says why; CI's jobs, timed, stay under their limit; CI is green.
- **How to test.** `cargo test -p slime-ui`; `cargo run --release -p slime-benches -- ui`.

### M6b.3 — Drawing the interface

- **Goal.** The renderer draws an interface's boxes, text and images, crisp at every scale.
- **Crates.** `slime-gfx`, `slime-render`, `slime-ui`, a new `slime-ui-render`; the examples.
- **Delivers.** In `slime-gfx`: a texture of one channel, writing a part of a texture, a scissor
  apart from the viewport, and, if the owner chooses to blend in sRGB's encoding, a view of the
  window's image without it, and blending of premultiplied colors if the canvas needs it. In
  `slime-render`, the canvas: rounded rectangles with their borders and shadows, glyphs, images and
  a view's target as an image, each clipped, with the indices of a transform and a layer that stay
  the identity and the root's until Stage 7; drawn after the views into any target, its GPU time in
  the statistics, its shader loaded again when it changes, made again after a lost device. In
  `slime-ui`, the display list. `slime-ui-render`, the bridge: a display list into a canvas, the
  glyphs' atlas pages, placed by etagere, within a budget, the least recently used leaving first,
  rasterized again after a lost device; images through the asset cache. An example draws paragraphs
  in several writing systems, boxes and a view in a panel, at scales 1 and 2, from a display list
  made by hand; CI builds it for Windows with the other programs. Whether the interface blends in
  linear space or in sRGB's encoding, chosen by the owner from pairs of images; the font of the
  editor's and the overlay's text, chosen by the owner (ADR-0133). The dependency etagere. A
  benchmark of the canvas's GPU time at the hardware tiers' resolutions
  ([ADR-0114](adr/0114-hardware-tiers-and-the-gpus-limits.md)). The ADR of the design, which sets
  the interface's share of a frame on each tier.
- **Done when.** Image tests: corners, borders and shadows; clips; glyphs of each writing system; an
  image, and a view drawn into a panel; scale 2; text drawn again after a lost device. The benchmark
  within the share of a frame that the ADR sets on each tier; CI is green.
- **How to test.** `cargo test -p slime-gfx -p slime-render -p slime-ui-render`; the example.
- **By hand.** The owner chooses how the interface blends and the font of its text, and judges the
  text at scales 1 and 2.

### M6b.4 — Elements, styles and layout

- **Goal.** An interface is a tree of elements, styled by themes read from files, laid out by
  flexbox and grid, and built again only where what it shows changed.
- **Crates.** `slime-ui`, `slime-ui-render`, `slime-cook`, `slime-assets`; the examples and the
  benchmarks.
- **Delivers.** As ADR-0132 decides: the tree of elements, handles of an index and a generation,
  keys among siblings and paths; roots, each with its size, its scale and its direction, that a
  window or later a texture holds; what changed in each element, its style, layout, paint or
  meaning; panels built again when a revision that they read moves, matched with the elements before
  by their keys. Typed styles; style sheets and themes as documents of reflected types, read by
  `slime-serialize`, with named values that rules share and rules selected by an element's kind,
  classes, key and states, cooked as scene files are and loaded again when they change
  ([ADR-0076](adr/0076-hot-reload-of-cooked-data.md)). taffy's flexbox and grid on the framework's
  tree; start and end rather than left and right. The display list built from what changed.
  Elements: a box, a label, an image, a button that shows its states, an area that scrolls, a list
  that builds only the rows that show. The dependency taffy. Benchmarks: a tree of the editor's size
  laid out, then laid out again after one change, then with nothing changed; a list of 100 000 rows
  scrolled and standing still. The ADR of the design.
- **Done when.** Tests without a GPU: layouts by flexbox and grid, mirrored from right to left; a
  panel built again keeps the state of its elements by their keys; a test that counts the layouts
  finds none in a frame where nothing changed; the list of 100 000 rows costs per frame what shows
  (benchmark, within its budget, written in the ADR); a theme changed on disk restyles a running
  example; image tests; CI is green.
- **How to test.** `cargo test -p slime-ui -p slime-ui-render`; the example, its theme changed while
  it runs.

### M6b.5 — Input, focus, and what agents see

- **Goal.** A person and an agent use the interface through the same actions, and an agent reads
  what it shows.
- **Crates.** `slime-ui`, `slime-app`, `slime-channel`, `slime-mcp`; the examples.
- **Delivers.** As ADR-0132 decides: the element under the pointer, from the last layout; the
  pointer held by what a button pressed; hovered and pressed states; input turned into actions and
  navigation intents, next, previous, a direction, accept, cancel; the interface's layers, popups
  and modals above panels, the top one taking input; focus scopes, Tab order and directions;
  shortcuts as data that name commands; what changes the world leaving as a command. The tree's
  meaning, each element's role, name, value, states, actions, bounds and path. A presentation gives
  its interface's tree, acts on an element and names the element at a pixel; the development
  channel's protocol carries them, and the MCP server's tools read the tree, find an element, act on
  it and capture it. The ADR of the design.
- **Done when.** Tests without a GPU: input turned into actions, then commands; focus moved by Tab
  and by directions, kept in its scope, given back when a layer closes; the tree's meaning as
  expected. An agent's session, in the windowed checks, finds a button by its role and its name,
  presses it, reads what changed in the world, and captures the element; CI is green.
- **How to test.** `cargo test -p slime-ui -p slime-app -p slime-channel`; the MCP server's session
  test.

### M6b.6 — Text edited, and what the platform lacks

- **Goal.** Text is typed, selected, copied and composed in every writing system, and screen readers
  read the interface.
- **Crates.** `slime-ui`, `slime-platform`; the examples.
- **Delivers.** A field of text, one line or several, on parley's editor: selection by the pointer
  and the keys, moves by word in text of both directions, an undo of its own, composition by input
  methods. A field of numbers that shows the value it holds whole and changes it only when the user
  changed it, by typing or dragging (G33's rule, which M6b.8 applies). In `slime-platform`: input
  methods, the clipboard through arboard, with its Wayland feature, cursor shapes, and the adapter
  of AccessKit, behind an `accessibility` feature, built into the editor and the examples and left
  out of games until Stage 7; the adapter exists before its window is shown, which changes how
  `slime-platform` makes windows. The dependencies arboard, accesskit and accesskit_winit; CI timed
  with the feature built. The ADR of the design.
- **Done when.** Tests without a GPU, with input methods and a clipboard simulated: text selected by
  the keys and the pointer, moves by word in text of both directions, the field's undo, a
  composition started, changed and committed, text copied and pasted, a field of numbers that the
  user did not change writing nothing; the tree's meaning given to AccessKit as expected; CI is
  green and its jobs under their limit with the feature built.
- **How to test.** `cargo test -p slime-ui -p slime-platform`; the example.
- **By hand.** Japanese typed through an input method on niri and on Hyprland, and in the Windows
  virtual machine; text copied and pasted between the example and other programs on Wayland and on
  Windows; the cursor's shapes over the example's elements; Orca reads the example on Linux, and
  Narrator on Windows.

### M6b.7 — Widgets and layers

- **Goal.** The framework has the widgets that the dev overlay and the editor are made of.
- **Crates.** `slime-ui`, `slime-ui-render`; the examples.
- **Delivers.** Check boxes, sliders, lists to choose from, sections that fold, tabs, tooltips,
  context menus, popups and modal layers, panels that float inside a window and are moved, folded
  and sized, splitters and scroll bars; each with its meaning and its actions. The editor's theme,
  and its fonts, of text, of fixed width and of icons
  ([ADR-0133](adr/0133-font-files-under-the-open-font-license.md)). An example shows every widget;
  CI builds it for Windows.
- **Done when.** Tests without a GPU of each widget's actions, meaning and focus; image tests of the
  example in the theme; an agent's session uses each widget through its actions; CI is green.
- **How to test.** `cargo test -p slime-ui -p slime-ui-render`; the example.
- **By hand.** The owner tries the example's widgets.

### M6b.8 — The dev overlay on the engine's interface

- **Goal.** The dev overlay runs on the engine's interface, and egui leaves the engine.
- **Crates.** `slime-devui`, `slime-gfx`; *Hunna Wood*, the `world` and `scene` examples;
  `deny.toml`.
- **Delivers.** As ADR-0132 decides: `slime-devui` rewritten on `slime-ui`, its world panel, which
  builds only the rows that show and keeps its selection by identity, and its editors of reflected
  values, booleans, numbers, text, structs, enums, options, lists, vectors, rotations as angles,
  colors, and references shown, which the editor's inspector of Stage 6c reuses; its fields change a
  value only when the user changed it. The panels that *Hunna Wood*, the `world` example and the
  `scene` example build with it, their statistics, time controls, views' settings and reports,
  rewritten on it too, its theme read from a file. The overlay takes the pointer and the keys as it
  does today, and outlives a lost device. A checklist of what the overlay does today, written at the
  milestone's start. egui and egui-wgpu leave the workspace, with `deny.toml`'s exception for their
  fonts and the overlay's use of `slime-gfx`'s `wgpu` feature. The ADR of the design.
- **Done when.** The checklist holds, by tests where a test is practical: the world panel of 100 000
  entities within its budget, retiring G46; a field that the user did not change writes nothing,
  retiring G33; the value editors send commands; the overlay released and restored after a lost
  device. An agent drives the overlay through the MCP server; `cargo deny` passes without egui's
  exception; CI is green.
- **How to test.** `cargo test -p slime-devui`; `cargo run -p hunnawood -- --new --save <file>`;
  `cargo run --example world`.
- **By hand.** The owner plays *Hunna Wood* with its F1 and F2 panels, uses the `world` and `scene`
  examples' panels, and listens to Orca read the `world` example's.

### M6b.9 — Windows check, documentation, release 0.6

- **Goal.** Stage 6b is done, checked on Windows, and released as 0.6.
- **Delivers.** The owner's check, in the Windows virtual machine, of the programs that CI builds,
  with the checks by hand that wait in [the list](#checks-by-hand-waiting-for-the-owner) if the
  owner makes them then; the documents reread as a newcomer would read them; `CHANGELOG.md`'s
  section of 0.6; the release, as [Releasing](#releasing) says, with the licenses of the fonts that
  the programs embed; Stage 6c planned with the owner.
- **Done when.** The owner ran *Hunna Wood*'s and the examples' overlays on Windows; the documents
  are reread and corrected; CI is green on the release commit; the tag and the GitHub release exist,
  with the Windows programs.
- **By hand.** In the Windows virtual machine: the overlay of *Hunna Wood* and of the `world`
  example; text in several writing systems in the examples; an input method and the clipboard in the
  examples' fields; Narrator reading the `world` example's overlay.

## Stage 6c — The editor

Exit criterion (see [`ROADMAP.md`](ROADMAP.md#stage-6c--the-editor--07)): a level of the game is
built, saved and reloaded entirely in the editor, with undo and redo on every operation; the editor
uses no private engine API. Released as 0.7 ([ADR-0131](adr/0131-stage-6b-and-stage-6c.md)).

The stage is planned in detail with the owner at Stage 6b's close. Its milestones, in an order
provisional until then:

1. **Docking, menus, dialogs, and drag and drop**
   ([ADR-0132](adr/0132-the-engines-ui-framework.md)): panels docked, in tabs, split, dragged from
   one place to another, their layout saved; a menu bar; dialogs to open and save, through rfd;
   drag and drop between panels; files dropped from the system on Windows and X11, and on Wayland
   once winit 0.31 is released.
2. **The editor's window and its viewport**: the editor's program, and *Hunna Wood*'s editor
   package, which M6b.1b makes ([ADR-0134](adr/0134-the-editors-core.md)), given a window; a view
   in a panel, drawn into a texture; its camera, with the raw motion of the mouse
   and the cursor held; the grid and the icons of lights; a click that selects, and the selection
   outlined, which needs the scene's depth resolved, and handles drawn over the image with a depth
   of their own ([Known debt](#known-debt)); when a viewport draws again; the modes of debugging
   that the editor needs; G34, with the editor's model of windows.
3. **Play in the editor** (ADR-0110): the game started in a child process on the level being
   edited, with a save of its own, in its own window; pause, speed, one step and stop; the game
   started again where it was after a change of its code; the world updated in place when a prefab
   changes while the game runs, keeping the handles that its code holds
   ([ADR-0089](adr/0089-scenes-and-prefabs.md)); the three seconds from Play to control, measured.
4. **The hierarchy and the inspector**: a tree of the world's entities that lays out only what
   shows, reparenting by drag and drop, as commands; an inspector driven by reflection, on M6b.8's
   value editors, overrides marked, applied and reverted; `EntityRef`, a reference to an entity of
   any scene, and its picker ([ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md));
   the rest of G32.
5. **Gizmos and placing**: moving, turning and scaling a selection; creating, duplicating and
   deleting.
6. **Levels of several files** (ADR-0112): parts of a level opened alone, former identities
   written when an entity moves out of its file, a prefab made of a selection among them
   ([ADR-0092](adr/0092-a-world-written-back-as-a-scene.md)); a valid change of a prefab that makes
   a level placing it refuse, checked before saving; whether a level that cannot spawn opens
   leniently, to be repaired (ADR-0089); G31, G40, G42, G43 and G45; whether a file says the parents
   and the order of a placement's entities, and keeps the comments of a file written by hand
   ([Known debt](#known-debt)).
7. **The asset browser**: the cooker's index of assets, thumbnails, assets dragged into the view,
   cooking from the editor; a load that a passing error failed tried again (G16); G20, G23, G49 to
   G53 and G56.
8. **Windows check, documentation, release 0.7.**

## Checks by hand waiting for the owner

The checks by hand of the milestones done that the owner has not made yet. The owner decided on
2026-10-20 to make them later, together: each milestone adds its own here when it is done, and a
check leaves the list once made, its result written in its milestone's outcome while the milestone
is in this plan, and in its stage's row of [Completed stages](#completed-stages) once it has left.
The game is run with `--new --save <a file of its own>`, so that the owner's save is left alone.

- **G20, in the Windows virtual machine.** A source asset whose `.meta` differs from its own name
  by case only, cooked twice: whether its ID changes at each cook, and whether `slime-cook watch`
  cooks it again without end (see [Known debt](#left-by-the-audit-of-04)).
- **P3, in the Windows virtual machine.** A timeout detection and recovery (TDR) forced while the
  zone loads, then a second while the game makes its GPU objects again: a new device each time,
  the log saying "the GPU device was lost; a new one replaces it", and the game going on.
- **P5, in the Windows virtual machine.** The benchmarks on Direct3D 12, `cargo run --release -p
  slime-benches -- render world loading`; the figures of WARP that the Windows job's log of
  `af512af` prints.

## Releasing

A release is an annotated git tag on a commit whose CI run is green on Linux and Windows
([ADR-0004](adr/0004-workspace-layout-and-crate-naming.md)):

1. In one commit, set the version in the root `Cargo.toml` and the tag that the README's "Using
   SlimeEngine in a game" names, rename the `[Unreleased]` section of `CHANGELOG.md` to
   `[<version>] - <date>`, and update this plan.
2. Push that commit and wait for its CI run to pass.
3. Tag that commit, not a later one, then push the tag:
   `git tag -a v<version> -m "SlimeEngine <version>" <commit>`.
4. From 0.1 on, create the GitHub release of the tag, its `CHANGELOG.md` section as its notes,
   with the `slime-examples-windows` artifact of that commit's CI run attached as
   `slime-examples-windows-<version>.zip`, holding beside the programs, and DXC's library and
   licence that CI puts with them, `LICENSE.txt`, a `NOTICE.txt` and the licenses of the egui fonts
   that `scene.exe`, `world.exe` and `hunnawood.exe` embed in their dev overlay, which the Ubuntu
   Font Licence asks to travel with them: CI keeps its artifacts 14 days, a release keeps them.

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
that commit.

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
| The audit pass and the foundations pass | P1 saves and identities that never mislead; P2 a damaged file gives an error, never a crash; P3 the loss of the GPU device and the main loop; P5 the build, CI, and measurements tracked from one commit to the next; P4 the measured quick wins; P6 the foundation decisions | Done on 2026-10-20, released with 0.5. The engine was audited at 0.4, without changing it, for correctness, measured performance and architecture against the games it aims at; [ADR-0097](adr/0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md) sums up what it found and records the owner's decisions, and [ADR-0096](adr/0096-the-files-the-engine-reads.md) is the threat model that ranks its defects. P1 to P5 fixed them ([ADR-0101](adr/0101-saves-and-identities-that-never-mislead.md) to [ADR-0106](adr/0106-the-measured-quick-wins.md)), but B4-F01, which M6a.2 fixed; P6 decided what the editor freezes ([ADR-0107](adr/0107-the-execution-model.md) to [ADR-0115](adr/0115-what-the-gpu-layer-gives-the-editor.md)). Each finding was described in full in this plan until 0.5 (`git show v0.5.0:docs/PLAN.md`); the audit's report and its tests are kept outside the repository; what neither pass fixed is in the [known debt](#left-by-the-audit-of-04); the checks by hand of P3 and P5 that wait are in [the list](#checks-by-hand-waiting-for-the-owner). |
| 6a — What the editor stands on → 0.5 | M6a.1 frustum culling and instancing; M6a.2 the engine's task pool, and copies to the GPU within a budget; M6a.3 what the world knows of its changes; M6a.4 a frame that owns what it draws; M6a.5 the phases of a frame, game time, and a game as a simulation and a presentation; M6a.6 the development channel and the MCP server; M6a.7a driven components, keys and created entities written into levels; M6a.7b saves per scene and adoption; M6a.8a views apart from the renderer; M6a.8b the settings of a view; M6a.9 asset requests; M6a.10a drawing for the editor; M6a.10b picking; M6a.11 Windows check, documentation, release 0.5 | Done, released as 0.5 on 2026-10-22 with the audit pass and the foundations pass. Its decisions are in [ADR-0116](adr/0116-stage-6-in-two-halves.md) to [ADR-0130](adr/0130-picking.md), its measurements in [`measurements.md`](measurements.md), under the milestones that measured; its save fixture is `games/hunnawood/fixtures/save.toml`, of the layout of saves per scene ([ADR-0125](adr/0125-saves-per-scene.md)). The owner checked each milestone that changed what shows by hand, on Linux, and the whole in the Windows virtual machine at M6a.11, which found four defects fixed before the release. |

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
  the mouse nor text; a run without a window has no image to capture; the measurements that the
  channel gives are the frames' times, not the renderer's statistics, which the presentation holds;
  an MCP server drives one game at a time; a component that refers to an entity without a
  persistent identity is read, the reference written as no entity, `""`, and written back so, no
  longer naming it. A window's image is created with the copy-source usage wherever the system
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
  sessions of the editor through the MCP server, on the yard and on the zone, are tests apart
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
  input-method text, touch, touchpad gestures, gamepads, dropped files, the mouse's raw motion. The
  window cannot grab or hide the cursor, go fullscreen, or change its title once created. M6b.6
  brings input methods and cursor shapes, Stage 6c dropped files, the raw motion and the cursor
  held, Stage 7 gamepads.
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
- **Upgrading wgpu waits for egui** while the dev overlay draws with it (ADR-0031; M6b.8).
  `egui-wgpu` is built against one wgpu version (egui 0.36 against wgpu 30 today), so wgpu moves
  only once egui follows. M6b.8 rewrites the overlay on the engine's UI framework and removes egui
  ([ADR-0132](adr/0132-the-engines-ui-framework.md)).
- **The dev overlay blends in an sRGB framebuffer.** egui prefers a linear one for its blending,
  warns once at startup, and its panels look slightly lighter than egui's own demos (ADR-0031;
  M6b.8). Harmless for tooling; it goes with egui at M6b.8.
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
- **The dev overlay can still panic on a lost device** (ADR-0022, ADR-0103; M6b.8, which rewrites it
  without egui). `Overlay::render` skips the frame when the device was lost earlier in it, and
  `Overlay::new` returns an error on a lost device, but two cases panic inside egui-wgpu or wgpu: a
  loss that egui-wgpu's own buffer upload is the first to find, and a device lost between
  `Overlay::new`'s check and egui-wgpu's creation of its buffers (`create_buffer_init`). Both are
  documented under `# Panics`; closing them needs a change in egui-wgpu or wgpu.
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
  ([ADR-0129](adr/0129-drawing-for-the-editor.md)). Instances over the image, which need a depth of
  their own, and the scene's depth resolved for an outline or handles partly hidden, come with the
  editor's handles at Stage 6c; a part of a texture written comes with the interface's glyphs at
  M6b.3. A line that lies on a surface may be hidden by it in places; blended surfaces are drawn in
  no particular order until Stage 9a sorts them. The `world` example selects with Tab from the
  keyboard alone: scripted keys reach a game's input, which its simulation reads, not its
  presentation's events. How long `Renderer::set_shaders` takes was measured before the unlit kind's
  pipelines and the lines'.
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
  a GPU rented in the cloud, a Steam Deck, or contributors would end it.
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
  the editor carries a run into them at Stage 6c, which meets it first.
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
  Stage 6c decides whether a file says them.
- **A scene file written back loses its comments** (ADR-0089): `World::write_scene` writes the
  canonical form. Stage 6c decides whether its editor keeps the comments of files written by hand.
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
  coordinate edited alone. The panel of M5a.6 edits a rotation whole.
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
  full it is).
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
  its size last changed. Stage 6c's viewports draw again then, or at every frame.
- **A render scale reads the scene's light bilinearly**
  ([ADR-0127](adr/0127-the-settings-of-a-view.md)): below 1 the image is blurred, and between 1 and
  2 each pixel averages the four texels nearest its centre, missing others. Stage 9a's temporal
  upscaling is the sharper way; a pass of a sharper filter was rejected.
- **Two modes of debugging** ([ADR-0127](adr/0127-the-settings-of-a-view.md)): the normals and the
  shadows. The editor adds those it needs, base color, light alone or cascades, at Stage 6c; a
  wireframe needs lines along a mesh's edges, which M6a.10a's drawings give a client to draw.
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
  Hot reload in the editor comes with Stage 6c.
- **A comment within a value written on several lines is lost** when the editor writes the level
  again, but for the end of the value's first line (`slime_serialize::keep_comments`).
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

- **A mesh that a passing error failed is not tried again** (G16, medium; Stage 6c). Since M6a.9,
  a mesh of an asset cache's request fails alone ([ADR-0128](adr/0128-asset-requests.md)), and is
  not asked for again until the cooked data changes, a passing error such as GPU memory running out
  included; a scene's load still fails whole. Trying a passing error again is to decide with the
  editor's asset browser.
- **On Windows, a `.meta` whose name differs from its source's by case only takes a new ID at every
  cook** (G20, medium; Stage 6c), and `slime-cook watch` cooks again without end: the scan matches
  the two files by the file system's rule, which ignores case on NTFS, then looks the ID up by the
  exact path. Not run on Windows by the audit: it waits in the
  [list of checks by hand](#checks-by-hand-waiting-for-the-owner).
- **Importing a glTF file takes time quadratic in its named meshes** (G23, medium; Stage 6c): 38 to
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
  medium; Stage 6c): it never counts the entities that the file gives once its placements resolve,
  which the reader refuses beyond `MAX_ENTITIES`, nor what they take, refused beyond
  `MAX_SCENE_SIZE`.
- **A very deep hierarchy costs quadratic time** (G32, medium; Stage 6c): spawning, loading, saving
  and each frame walk every entity's ancestors (`set_parent`'s search for a cycle,
  `World::unsaved`, `interpolated_transform`). M6a.3 made `World::is_hidden` read a flag.
- **The world panel rounds a number that was only clicked or crossed with the keyboard, and writes
  the rounding into the world** (G33, medium; M6b.8): egui's `DragValue` shows as many decimals
  as the drag's speed gives, and writes back the text it shows when it loses the focus. The rule for
  the editor's inspector: a field changes a value only when the user changed it.
- **A GPU object used with another `Gpu` acts on another object without a word, and no API opens a
  second window on the same GPU** (G34, medium; Stage 6c): each `Gpu` numbers its objects from zero,
  and a window's surface comes only with a new `Gpu`. The editor's model of windows decides.
- **Between two fixed steps, the children of a turning parent cut the arc's chord while their
  siblings stay on it** (G38, low; Stage 7): a child moved by its parent is drawn in a straight line
  between its two positions. Interpolating through the hierarchy is to decide before skeletal
  animation turns many hierarchies.
- **A string on several lines in a scene file checked out on Windows keeps its CRLF line ends**
  (G40, low; Stage 6c): `.gitattributes` keeps `*.toml`, `*.meta` and `*.tsv` at LF, not `*.scene`,
  so that the value and the cooked bytes depend on the checkout.
- **A negative integer, or one too large, for an unsigned field gives a wrong message** (G42, low;
  Stage 6c): "not an integer" or "not text", rather than its range.
- **The writer of documents accepts what the reader refuses** (G43, low; Stage 6c): a reserved key
  `$…` at the root, and a field given twice, the second lost.
- **A camera target farther than about 2 × 10¹⁹ m draws nothing, without a message** (G44, low;
  later): `view_from_eye` squares the direction in `f32`.
- **`World::write_scene` loops without end when more than 1 679 616 entities need IDs from one
  base** (G45, low; Stage 6c): it draws four base-36 characters until one is free.
- **The world panel costs about 1.25 µs per entity and per frame** (G46, low; M6b.8): 125 to 145
  ms per frame at 100 000 entities, every row laid out, visible or not, measured before M6a.3,
  which makes the list again only when the world's structure moves, and was not measured again.
  The editor's hierarchy needs a virtualized list.
- **`Registry::register_dynamic` accepts version 0, which the derive refuses** (G48, low; Stage 8):
  the values are written, then never read back, with a wrong message.
- **Two cooks in one process share their temporary files** (G49, low; Stage 6c): named after the
  process, they separate processes, not threads, and one cook fails at random, naming a file that
  exists.
- **An output directory inside the sources breaks the next cook** (G50, low; Stage 6c): cooked glTF
  scenes take the `.scene` extension of scene files, which the next scan reads as sources.
- **After a glTF file is moved with its `.meta`, the cooker's cache keeps the old path** (G51, low;
  Stage 6c): the manifest names it, and warnings blame tables that are right.
- **Two `.meta` with one ID, one of them without a source, make the cook stop without saying which
  is the orphan** (G52, low; Stage 6c).
- **`Manifest::from_bytes` refuses a valid manifest whose names are short** (G53, low; Stage 6c): it
  assumes 45 bytes per entry where the least is 37.
- **With `--watch`, after the cooked data is cleaned, a reload that the game refuses leaves it
  unable to save** (G56, low; Stage 6c): `SceneLibrary::reload` forgets the changed scenes before
  the game accepts them, and the files of the old manifest are gone.
- **The dev overlay is built into the game that ships**, egui and its fonts with it (B1-F08;
  measured again at M6b.8, which rewrites it without egui): an estimated 10 s of the game's release
  build, 46 s on the owner's machine. The owner chose on 2026-10-19 to keep it in the game, and its
  fonts with it (P5); building it only in development would need a feature of the game.

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
