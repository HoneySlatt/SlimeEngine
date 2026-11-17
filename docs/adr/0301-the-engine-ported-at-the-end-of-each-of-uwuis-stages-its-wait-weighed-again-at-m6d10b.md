# 0301. The engine ported at the end of each of uwui's stages, its wait weighed again at M6d.10b

- Status: Accepted; supersedes
  [0277](0277-the-engine-ported-to-uwui-at-the-end-of-each-of-uwuis-stages.md)
- Date: 2026-11-17

## Context

[ADR-0276](0276-the-engine-follows-uwui-by-one-exact-commit-until-its-1-0.md) has the engine name
one exact commit of uwui until uwui's 1.0; [ADR-0277](0277-the-engine-ported-to-uwui-at-the-end-of-each-of-uwuis-stages.md)
set how often the engine moves it: at the end of each of uwui's stages.

- No stage of the engine runs between the move and uwui's 1.0: Stage 6e, the end of Stage 6c and
  Stage 7 wait for it ([`ROADMAP.md`](../ROADMAP.md#uwuis-stages)).
- uwui's road to its 1.0 is long, and no document bounds it: its first stage, then the rest of its
  library, its platforms and its tools, cut into stages by
  [M6d.10b](../stage-6d.md#m6d10b--uwuis-first-stage-planned).
- Meanwhile the engine's contracts of execution stay undecided: the simulation's
  ([ADR-0287](0287-the-simulations-contract-designed-at-stage-7-its-questions-written-now.md)), a
  host without a window, one budget of memory; and uwui's API is frozen by its semantic versioning
  at its 1.0 with no game among its clients: a menu driven by a gamepad, an interface in the world,
  drawing in the render graph.
- ADR-0277 asked for another decision only if ports outgrew a milestone twice: nothing weighed the
  wait itself. A read-only review of the engine's architecture, on 2026-11-17, found that gap.

## Decision

The owner's decision of 2026-11-17. With it, the rule in force on how the engine follows uwui's
stages before uwui's 1.0:

**At the end of each of uwui's stages, a milestone of the engine moves its commit of uwui** to that
stage's last, ports the editor, the games and the examples to it, and takes the versions of wgpu,
accesskit and rmcp that uwui then uses. Between two stages' ends, the engine moves its commit only
for a fix that it needs. The first of these milestones follows uwui's first stage.

**A port that outgrows a milestone is reported to the owner** before it goes on, with what it found
of uwui's API.

**The wait is weighed again at M6d.10b.** Once uwui's stages to its 1.0 are cut, the owner weighs one
choice: whether Stage 7's design session and its trial on one case, which ask nothing of uwui, run
during uwui's stages, or the order stays. The choice is recorded in an ADR either way.

## Alternatives considered

- **ADR-0277 as it stands:** a plain plan, but a long wait for uwui confronted with nothing, and the
  engine's contracts undecided for as long.
- **Stage 7's design session and its trial brought forward now,** beside uwui's first stage: the
  contracts of execution settled early and uwui's API seeing a game's needs before it freezes, but
  against the order the owner kept twice, a trial on the identities that Stage 6c's rest changes,
  and a third track beside the two at most.
- **The engine frozen at the move's commit until uwui's 1.0, then ported once,** and **a port at
  each milestone of uwui:** rejected as ADR-0277 rejected them, a large port at once, or the editor
  ported at each change of the API ([ADR-0194](0194-stage-6d-uwui-leaving-the-engine.md)).

## Consequences

- A few milestones of the engine run during uwui's stages, one at each stage's end; the engine's
  plan holds each when its stage of uwui ends.
- The editor tries uwui's API before its 1.0 freezes it; what it finds goes to uwui's next stage.
- The weekly job of ADR-0276 tells how far the engine is behind between two stages' ends; being
  behind there is expected, not a fault.
- M6d.10b gains one choice for the owner.

## Validation and reconsideration

Read: both roadmaps' order of stages; ADR-0276, ADR-0277 and ADR-0287. Assumed: that a port at a
stage's end fits a milestone. Proposing another decision is justified if ports outgrow a milestone
twice, if uwui's stages come so often that the ports cost more than the stages that they follow, or
at M6d.10b, as above.
