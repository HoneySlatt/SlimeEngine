# 0131. Stage 6b, the engine's interface, and Stage 6c, the editor

- Status: Accepted; amends [0116](0116-stage-6-in-two-halves.md) (Stage 6b split again, and every
  later stage released one version later), [0090](0090-stage-5b-scenes-and-prefabs.md) (the editor
  is released as 0.7), [0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md) (the
  release map) and [0098](0098-the-games-the-engine-aims-at.md) (Stage 9c's work ships with 0.12);
  amended by [0134](0134-the-editors-core.md) (the first milestone in two, and *Hunna Wood*'s editor
  package in it)
- Date: 2026-10-22

## Context

[ADR-0116](0116-stage-6-in-two-halves.md) split Stage 6 in two halves: Stage 6a, what the editor
stands on, released as 0.5 on 2026-10-22, and Stage 6b, the editor, to be released as 0.6. It left
Stage 6b to be planned in detail at Stage 6a's close, with a study of the UI framework's design
([ADR-0098](0098-the-games-the-engine-aims-at.md)), "and split again then if it needs to be".

The owner chose on 2026-10-22 how the study was made: four agents, each alone, proposed a design
from another starting point (what other engines learned, the Rust ecosystem, this engine's own
architecture, the needs of the games of ADR-0098), and a fifth measured the candidate crates on the
owner's machine. Their design is recorded in [ADR-0132](0132-the-engines-ui-framework.md). What
they found decides the size of the stage:

- **The framework alone is larger than the plan said.** Stage 6b's outline counted it as two or
  three milestones. The four proposals count six to nine, of 18 000 to 25 000 lines of Rust with
  their tests, where each of Stage 6a's milestones added about 1 000 to 4 000, as their commits
  show: text in several writing systems, its drawing, elements, styles and layout, input and focus,
  text editing and the platform's services, widgets, docking.
- **With the editor's own eight milestones**, the core, the window and its viewport, play, the
  hierarchy and the inspector, gizmos, levels of several files, the asset browser and the release,
  Stage 6b comes to fourteen to eighteen milestones by the proposals' counts. Stage 6a had eleven,
  grown to fourteen by three splits; Stage 4 was split at about fourteen
  ([ADR-0068](0068-stage-4-in-two-halves.md)), Stage 5 at twelve or thirteen
  ([ADR-0078](0078-stage-5-in-two-halves.md)). All four proposals recommend a split.
- **The framework gains a user in the games before the editor.** The owner decided on 2026-10-22
  that the dev overlay is rewritten on the framework as soon as its base widgets are ready, and egui
  removed then (ADR-0132). ADR-0116 had rejected three parts, "the engine, then the editor's core
  with the UI framework, then the panels and play", for "a UI framework built without the panels
  that use it, against the rule of a second concrete use, and a middle part of which a game sees
  nothing": with the overlay, *Hunna Wood* and the `world` example are the framework's users, in the
  programs that ship.

The owner took the decision on 2026-10-22, among the options that "Alternatives considered" lists.

## Decision

**Stage 6b is split in two, each released.** A reference to Stage 6b in a document written before
this ADR, where it means the editor, means Stage 6b and Stage 6c; [`docs/PLAN.md`](../PLAN.md)
says which part each piece of work, and each item of the known debt, goes to.

- **Stage 6b, The engine's interface, released as 0.6**, in nine milestones, in this order, each
  after what it needs, the riskier first:
  1. The editor's core, without its interface, as ADR-0116 decided: the editor's commands, which
     name entities by their persistent identities, the journal, undo and redo, the selection, a
     level opened and written back, all of which the MCP server gains at once
     ([ADR-0109](0109-stable-references-and-undo.md)).
  2. Text in several writing systems, without a GPU.
  3. Drawing the interface.
  4. Elements, styles and layout; themes read from files; lists that lay out only what shows.
  5. Input, focus, and what agents see of the interface.
  6. Text edited, and what the platform lacks for it: input methods, the clipboard, cursors, and
     screen readers.
  7. Widgets and layers.
  8. The dev overlay rewritten on the framework, and egui removed.
  9. A check on Windows, the documents reread, and the release of 0.6.

  Its exit: the dev overlay of *Hunna Wood* and of the `world` example runs on the engine's UI
  framework, on Linux and on Windows, its theme read from a file, and the framework draws text in
  several writing systems; an agent reads and drives the overlay's panels through the MCP server,
  and a screen reader reads those of the `world` example; the list of a world of 100 000 entities
  stays within the budget that Stage 6b sets for it; egui is gone; and the editor's core changes a
  level through commands, undone and redone, written back and opened again, driven by an agent.
- **Stage 6c, The editor, released as 0.7**: docking, menu bars, dialogs, and drag and drop; the
  editor's window and its viewport, with *Hunna Wood*'s editor package; play in the editor; the
  hierarchy and the inspector; gizmos and placing; levels of several files; the asset browser; a
  check on Windows, the documents and the release. Its exit is Stage 6's, which ADR-0116 gave Stage
  6b: a level of the game is built, saved and reloaded entirely in the editor, with undo and redo
  on every operation, and the editor uses no private engine API. It is planned in detail at Stage
  6b's close, in the order of ADR-0116, which stays provisional until then.
- **Every later stage is released one version later**: Stage 7 as 0.8, Stage 8 as 0.9, Stage 9a as
  0.10, Stage 9b as 0.11, Stage 10a as 0.12, with Stage 9c's work, Stage 10b as 0.13, and Stage 11
  as 0.14, then 1.0.

## Alternatives considered

- **One Stage 6b, released as 0.6 at its end**, as ADR-0116 planned: no release number moves, but
  the longest stage of the project, seventeen milestones in this plan's order without a release or a
  check of the whole on Windows, which ADR-0116 split Stage 6 to avoid.
- **Stage 6b up to play in the editor**: the editor's core, the framework with docking, the editor's
  window and viewport, and play, released as 0.6; the hierarchy and the inspector, gizmos, levels
  and the asset browser as Stage 6c. Three proposals of the study suggested a split close to it. 0.6
  would show an editor's window, and ADR-0110's three seconds from Play to control would be measured
  sooner; but twelve milestones against five, nearly as many as Stage 6a's fourteen, and a second
  half too short to be worth a release of its own.
- **The framework alone first**, without the editor's core or the overlay: the framework built for
  no user for months, which ADR-0116 rejected.

## Consequences

- 0.6 ships the engine's UI framework, the dev overlay on it, and the editor's core; 0.7, the
  editor. Every stage after Stage 6 is released one version later than ADR-0116 said, and 1.0
  follows 0.14.
- [`docs/ROADMAP.md`](../ROADMAP.md) shows Stage 6b and Stage 6c, each with its exit, and the
  release map moved by one; [`docs/PLAN.md`](../PLAN.md) shows Stage 6b's nine milestones and Stage
  6c's outline.
- ADR-0110's three seconds from Play to control are measured at Stage 6c.
- Each milestone that designs a part records its design in a new ADR, which amends the ADR that
  decided the part where it changes it, as ADR-0116 said.
