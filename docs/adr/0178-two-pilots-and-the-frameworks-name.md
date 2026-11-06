# 0178. Two pilots, and the framework's name

- Status: Accepted; amends [0170](0170-applications-on-the-ui-framework.md) (both programs ported
  during Stage 6d; the framework named now, in a repository of its own once its drawing leaves the
  renderer) and [0004](0004-workspace-layout-and-crate-naming.md) (the framework's crates are not
  named `slime-<role>`); amended by [0179](0179-the-framework-named-uwui.md) (the framework named
  uwui), [0180](0180-one-canvas-and-the-frameworks-foundations.md) (what leaves with the
  framework, and when its repository moves)
  [0181](0181-uwuis-scope.md) (four programs before the API is declared stable) and
  [0194](0194-stage-6d-uwui-leaving-the-engine.md) (the pilots ported in uwui's first stage)
- Date: 2026-11-05

## Context

[ADR-0170](0170-applications-on-the-ui-framework.md) made the engine's UI framework run the
owner's desktop programs at Stage 6d. It took the launcher of games as the stage's pilot, ported
during the stage, and the music player after it, its indispensable needs met by the stage's exit.
It kept the framework in this repository, under the engine's name, until the day it is published
for other people.

Two things weigh against that:

- The music player carries the effects that the stage is hardest on: glass, shapes that morph,
  springs, transitions, lyrics colored word by word, covers that move
  ([ADR-0171](0171-effects-of-the-interface.md)). The launcher proves lists, images and scrolling
  first. The music player is also the program the owner uses most.
- The owner's future programs, unrelated to games, are to build on the framework. A name of the
  engine on it says otherwise, and renaming costs more each week, as the framework and its users
  grow.

What a repository of its own would cost now, measured on 2026-11-05:

- `slime-ui`, about 30 000 lines, depends on four of the engine's crates: `slime-color`,
  `slime-input`, `slime-reflect` and `slime-serialize`.
- Drawing it goes through `slime-ui-render` and the renderer's canvas: `slime-render`, about
  28 000 lines, with its assets and its tasks, and `slime-gfx`, about 12 000. A repository of the
  framework would depend on the engine's to draw, and the engine's on it for its interface.
- Of the 85 commits that changed `slime-ui` since 2026-10-20, 59 changed other crates too.

## Decision

**Two pilots.** Stage 6d ports both programs onto the framework during the stage, the music player
and the launcher, each in its own repository, each with a checkpoint where the owner uses it. The
music player's figures with its present framework, its start, its frames while it scrolls and
animates, its memory, are measured before the stage, as the launcher's were. The exit asks both to
run at least as well as with their present frameworks, on each of those figures. The order of the
two ports is planned at the stage's start.

**The framework's name: lumui.** Its crates are renamed now, in this repository, in a milestone of
their own after M6c.Q1: `slime-ui` becomes `lumui`, and `slime-ui-render`, `lumui-render`. In
prose, the framework is lumui. The engine's other crates keep the `slime-<role>` names of
[ADR-0004](0004-workspace-layout-and-crate-naming.md); earlier ADRs keep the names of their date.

**A repository of its own at Stage 6d,** once its drawing has left the renderer: an application
that draws no scene does not pay for the 3D renderer (ADR-0170), and the framework, then standing
on base crates that move with it, leaves without the two repositories depending on each other.
Its boundary stays ADR-0170's: it depends on neither the world, a game's host, the editor nor
games.

## Alternatives considered

- **The launcher alone as the pilot**, as ADR-0170 decided: a smaller port, figures already
  measured, but the effects proven by examples alone, and the music player waiting for the stage's
  end.
- **The music player alone as the pilot**: the hardest effects proven, but the launcher's lists of
  thousands of games and their images proven by examples alone.
- **A repository and a name now**: three to five sessions of moving, estimated, before the editor
  goes on; two repositories that depend on each other until the canvas leaves the renderer; seven
  changes of the framework in ten made in two repositories, while it changes most.
- **The name and the repository at Stage 6d's end**: nothing now, but more code to rename then.
- **`magnolia`**: the name of another program of the owner's, and taken on crates.io.

## Consequences

- Stage 6d is longer, by the music player's port.
- A milestone renames the framework's crates before M6c.Q2a; the documents name it lumui.
- The music player's own plan changes: it is ported during the stage, not after it.
