# 0116. Stage 6 in two halves

- Status: Accepted; amends [0090](0090-stage-5b-scenes-and-prefabs.md) (the editor is released as
  0.6), [0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md) (the passes ship with
  0.5, Stage 6a's release, and every later stage is released one version later),
  [0098](0098-the-games-the-engine-aims-at.md) (Stage 9c's work ships with 0.11),
  [0109](0109-stable-references-and-undo.md) (a created entity carried once before the editor's
  command system) and [0110](0110-how-the-editor-runs-a-games-code.md) (the development channel
  built before play in the editor, with the MCP server); amended by
  [0124](0124-driven-components-keys-and-created-entities-in-levels.md) (M6a.7 in two milestones)
- Date: 2026-10-20

## Context

The foundations pass ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md),
[`docs/PLAN.md`](../PLAN.md), P6) decided, in [ADR-0107](0107-the-execution-model.md) to
[ADR-0115](0115-what-the-gpu-layer-gives-the-editor.md), what the editor will freeze, and when
Stage 6 builds each part; [ADR-0098](0098-the-games-the-engine-aims-at.md) adds the engine's own UI
framework, and [ADR-0099](0099-ai-agents-drive-the-engine.md) the engine's MCP server. P6 is done
when Stage 6 is planned with the owner on them.

- **Stage 6 is the largest stage so far.** Planned on these decisions, it comes to about 21
  milestones: frustum culling and instancing; the engine's task pool and the copies to the GPU
  within a budget; the world's ticks; the frame that owns what it draws; the phases of a frame, game
  time and a game split into a simulation and a presentation; the development channel and the MCP
  server; identities and saves per scene; views; asset requests; rendering for the editor; the
  editor's commands, journal and undo; the UI framework, in two or three milestones; the editor's
  window and viewport; play in the editor; the hierarchy and the inspector; gizmos; levels of
  several files; the asset browser; and the checks on Windows and the releases. Stage 4 was split
  at about fourteen milestones ([ADR-0068](0068-stage-4-in-two-halves.md)), Stage 5 at twelve or
  thirteen ([ADR-0078](0078-stage-5-in-two-halves.md)); each of Stage 5b's first six milestones
  added 850 to 2 830 lines of Rust, code and tests.
- **Nothing has been released since 0.4**, on 2026-10-18. The audit pass fixed defects that no
  release holds: critical ones, such as a save applied to other entities than those it names
  ([ADR-0101](0101-saves-and-identities-that-never-mislead.md)), and others, such as files that
  crash the engine ([ADR-0102](0102-a-damaged-file-gives-an-error.md)). ADR-0097 ships both passes
  with 0.5, "the editor's release", at the end of Stage 6.
- **The agents that build the engine do not see it run.** They reach a running game through tests,
  logs and captures of their own; what shows on screen waits for the owner's checks by hand, which
  ADR-0099 names as what slows such work the most. The roadmap lists the MCP server after play in
  the editor, at the end of Stage 6, and [ADR-0110](0110-how-the-editor-runs-a-games-code.md)
  builds the development channel with play in the editor.
- **The decisions depend on each other.** [ADR-0111](0111-the-frame-contract.md)'s frame that owns
  what it draws makes the last two phases of ADR-0107's frame two calls;
  [ADR-0113](0113-asset-requests-residency-and-priorities.md)'s requests need ADR-0107's owner of
  the copies to the GPU and ADR-0111's counted references; ADR-0115's rendering for the editor comes
  after culling, instancing and ADR-0111's views; [ADR-0109](0109-stable-references-and-undo.md)'s
  commands come before any panel that changes the world;
  [ADR-0108](0108-what-the-world-knows-of-its-changes.md)'s driven mark and ADR-0109's created
  entity carried once come before the editor writes levels.

The owner took five decisions on 2026-10-20, one at a time, each among the options that
"Alternatives considered" lists.

## Decision

**Stage 6 is split in two halves, each released.** A reference to Stage 6 means both.

- **Stage 6a, What the editor stands on, released as 0.5**, with the audit pass and the
  foundations pass: what the editor stands on, built and checked without the editor's interface,
  in eleven milestones, below. Its exit: *Hunna Wood* runs on it, on Linux and on
  Windows. What the camera does not see is not drawn, and the rest is drawn by instances, measured
  at the hardware tiers' resolutions against their budgets
  ([ADR-0114](0114-hardware-tiers-and-the-gpus-limits.md)); its zone loads without a frame missing
  its time; its game time pauses and steps; an agent drives it through the engine's MCP server,
  pausing and stepping it, reading and changing its world, capturing its image and playing it with
  scripted input; its saves are differences per scene, which cost what changed. The `world` example
  draws a frame in several views and picks the object under the cursor.
- **Stage 6b, The editor, released as 0.6**: the editor's core, the UI framework, the editor's
  panels and play in the editor. Its exit is the roadmap's exit of Stage 6: a level of the game is
  built, saved and reloaded entirely in the editor, with undo and redo on every operation, and the
  editor uses no private engine API. It is planned in detail at Stage 6a's close, with a study of
  the UI framework's design (ADR-0098), and split again then if it needs to be.
- The stages after Stage 6 are each released one version later: Stage 7 as 0.7, Stage 8 as 0.8,
  Stage 9a as 0.9, Stage 9b as 0.10, Stage 10a as 0.11, with Stage 9c's work, Stage 10b as 0.12,
  and Stage 11 as 0.13, then 1.0.

**Stage 6a, in this order**, each milestone after what it needs, the riskier first:

1. Frustum culling and instancing (ADR-0097), with the pieces of the GPU layer that they need
   (ADR-0115), and the benchmarks measured at the hardware tiers' resolutions (ADR-0114).
2. The engine's task pool, and one owner of the copies to the GPU within a budget of bytes per
   frame (ADR-0107).
3. The world's ticks, components that clone and the flag of entities hidden by an ancestor
   (ADR-0108), before the examples, the game and the benchmarks are rewritten, so that they are
   rewritten once, on the guards through which ADR-0108 gives mutable access.
4. The frame that owns what it draws, the asset cache without the renderer, kinds of material
   (ADR-0111) and the floor of the GPU's limits (ADR-0114), before the split of a game, so that a
   presentation is written once, in the two calls that ADR-0111 gives it: extraction, then
   rendering.
5. The phases of a frame, game time, and a game as a simulation and a presentation run by a host
   (ADR-0107). *Hunna Wood* becomes a library and its program (ADR-0110); the dev overlay's edits
   become the first command, which names its entity by its persistent identity (ADR-0109) and is
   applied at the start of a frame; the overlay pauses and steps the game.
6. The development channel and the engine's MCP server (ADR-0110, ADR-0099), before play in the
   editor, where ADR-0110 had placed the channel: agents drive a running game from here on, and
   each later milestone exposes its system to the server as it is written.
7. Identities and saves per scene: ADR-0108's driven mark; ADR-0109's keys of what a start creates
   in code, and its created entity written into a level and carried once, ahead of the editor's
   command system; [ADR-0112](0112-levels-saves-and-worlds-for-open-worlds.md)'s saves made of
   differences per scene, what persists, and adoption. It comes before the editor writes levels, and
   is split in two within Stage 6a if it outgrows a milestone: first the driven mark, the keys and
   the carried entity, then saves per scene and adoption.
8. Views apart from the renderer (ADR-0111).
9. Asset requests, residency and priorities (ADR-0113).
10. Rendering for the editor (ADR-0115).
11. A check on Windows, the documents reread, and the release of 0.5.

**Stage 6b begins with the editor's core, without its interface**: the editor's commands, which
name entities by their persistent identities, its journal, undo and redo, its selection, a level
opened and written back (ADR-0109), all of which the MCP server gains at once. The UI framework
follows (ADR-0098); then the editor's window and its viewport; play in the editor, early enough to
measure ADR-0110's three seconds; the hierarchy and the inspector, with `EntityRef` and its picker
(ADR-0112); gizmos and placing; levels of several files, with former identities (ADR-0112); and the
asset browser. This order is provisional until Stage 6b is planned.

**The rest.** The development channel's protocol is versioned (ADR-0110), and Stage 6b adds the
editor's messages to it. *Hunna Wood*'s editor, the package `games/hunnawood-editor`, comes with
Stage 6b's window. Stage 6a asks for one dependency known today, an implementation of MCP for the
server, at its milestone, with the alternatives considered; whether the channel's token needs one
is decided there (ADR-0110); the crate of the task pool uses the standard library alone. The
defects of the audit of 0.4 that wait for Stage 6 go to the milestones that
[`docs/PLAN.md`](../PLAN.md) names.

## Alternatives considered

The shape of the stage:

- **One stage, released as 0.5 at its end**, as the roadmap said: nothing to change, but about 21
  milestones without a release or a check of the whole on Windows, the audit pass's fixes
  unreleased until the editor is done, and the editor planned in detail before the engine under it
  is built.
- **Two halves, one release at the end**, Stage 6a without a release of its own, as Stage 4b had
  none: release numbers unchanged, but the fixes wait as long.
- **Three parts**, the engine, then the editor's core with the UI framework, then the panels and
  play: smaller parts, but a UI framework built without the panels that use it, against the rule of
  a second concrete use, and a middle part of which a game sees nothing.

The channel and the MCP server:

- **At the end of Stage 6**, after play in the editor, in the roadmap's order: designed with every
  user known, but the agents build Stage 6a and most of Stage 6b without seeing what they build
  run, and the systems built meanwhile are exposed to the server afterwards, at the higher cost for
  which ADR-0099 rejected waiting for Stage 8 or Stage 11.
- **At the start of Stage 6b**, beside the editor's commands: designed together, but Stage 6a's
  rendering, where seeing the image matters most, is built without it, and 0.5 ships without it.

Identities and saves per scene:

- **In Stage 6b, with levels of several files**: files and saves designed together, with the editor
  as their user; but the driven mark and the carried entity would still have to come before the
  editor writes levels, 0.5 would ship the save model that changes, a start world made again at each
  save, and the riskiest work would come at the end of the stage.
- **Split, the driven mark, the keys and the carried entity in Stage 6a, saves per scene and
  adoption in Stage 6b**: smaller pieces, but the code of saves changed twice, likely in two
  layouts.

The frame that owns what it draws and the split of a game:

- **The split of a game first**: the riskier change, and game time, one milestone sooner; but a
  presentation of one call, then of two, every program changed twice.
- **Both in one milestone**: one pass over every program, but some 5 000 lines, more than any
  milestone so far, hard to review and to check.

The editor's core:

- **The UI framework first**: a window sooner, but the framework designed for a demonstration, and
  the core under the pressure of the panels that wait for it.
- **Each panel bringing its commands**: each command with its first user, but panels built before
  what they rest on, the risk that the audit named, and undo and the journal designed for one
  panel, then generalized.

## Consequences

- 0.5 ships the audit pass, the foundations pass and Stage 6a; 0.6, the editor. Every stage after
  Stage 6 is released one version later, and 1.0 follows 0.13.
- [`docs/PLAN.md`](../PLAN.md) shows Stage 6a's eleven milestones and Stage 6b's outline; P6 is
  done.
- From Stage 6a's sixth milestone on, agents check on screen what they change, through the MCP
  server; the owner's checks by hand go to the look and the feel of the game, and to Windows.
- *Hunna Wood*'s saves take a new layout in Stage 6a, and its fixture is made again, which
  [ADR-0100](0100-updates-of-shipped-games.md) allows before the game's first release.
- Each milestone that designs a part records its design in a new ADR, which amends the ADR that
  decided the part where it changes it.
- [`docs/ROADMAP.md`](../ROADMAP.md) shows Stage 6a and Stage 6b, each with its exit, and the
  release map moved by one.
