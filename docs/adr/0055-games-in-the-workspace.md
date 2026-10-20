# 0055. Games in the workspace

- Status: Accepted; amends [0004](0004-workspace-layout-and-crate-naming.md); amended by
  [0067](0067-hunna-wood-replaces-nightwatch.md) (the first game replaced) and
  [0110](0110-how-the-editor-runs-a-games-code.md) (a game is a library, its program and its
  editor)
- Date: 2026-10-14

## Context

Stage 3c makes a first game, *Nightwatch*, with the engine's public API only, and from then on the
sample that every stage ends with is that game, grown by the stage
([ADR-0052](0052-amend-roadmap-before-0-1.md)). [ADR-0004](0004-workspace-layout-and-crate-naming.md)
puts the engine's crates in `crates/` and the runnable examples in one `examples/` package, each
example a file; it creates other top-level directories when they have content, and names none for
games.

A game is not an example: it is many files that grow over the stages, it has its own rules and
tests, and it uses the engine as a project outside it would, without the examples' shared code. It
must still build and run in every check, on Linux and Windows, so that a change of the engine that
breaks it is seen at once.

The owner decided on 2026-10-14, when Stage 3c was planned, that the game lives in the workspace.

## Decision

- Games live in `games/<package-name>/`, one package each, members of the workspace: the first is
  `games/nightwatch`, the package `nightwatch`.
- A game is a binary, not published, that depends on the engine's crates by path, through the
  workspace's dependencies, and uses their public API only: no `wgpu` feature, no copy of the
  examples' code. What it cannot do through that API is recorded as a finding of the stage, not
  worked around inside the engine.
- No engine crate depends on a game.
- `scripts/check.sh` and CI build, test and lint games with the rest of the workspace, run them for
  a number of frames in the headless Weston, and CI builds them for Windows beside the examples.
- A game's package name is its own, without the `slime-` prefix that ADR-0004 gives the engine's
  crates: a game is a client of the engine, not a part of it.

This amends ADR-0004 (a top-level `games/` directory, and packages named without the prefix).

## Alternatives considered

- **An example in `examples/`.** No change of layout, but the examples are one file each sharing
  `examples/common/`, and a game that grows over the stages would crowd a package meant for small
  demonstrations, with access to the examples' code that another project would not have.
- **A repository of its own.** The strongest proof that it uses the public API only, but every
  change of the engine's API would have to be followed in a second repository, and the checks of
  the engine would not see a change that breaks the game.

## Consequences

- The workspace builds one more binary in every check and CI run.
- A change of the engine's public API updates the game in the same change, as it updates the
  examples.
