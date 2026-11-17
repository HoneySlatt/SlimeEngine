# 0300. M6d.10 cut in two: what the move needs before it, uwui's first stage planned after

- Status: Accepted
- Date: 2026-11-17

## Context

[M6d.10](../stage-6d.md#m6d10--uwui-designed-as-a-project) designs uwui as a project before its code
moves: about 25 decisions for the owner, from its crates' names to what four programs ask of its
first stage, its platforms, the services that the web and the phones lack, and its rule for errors.
D1, M6d.11 and M6d.12 follow it, alone ([`PLAN.md`](../PLAN.md#current)): from M6d.10 to the end of
the stage, neither track goes on, and [E1](../stage-6d.md#e1--the-elements-contract) now comes before
it ([ADR-0294](0294-the-elements-contract-designed-by-e1-before-m6d10-on-a-prototype-thrown-away.md)).

A read-only review of the engine's architecture, on 2026-11-17, sorted those decisions: a few
change what leaves, its names or its CI, and the move needs them; most plan uwui's first stage,
whose ADRs would be written here, consolidated by D1, then moved.

## Decision

The owner's decision of 2026-11-17.

**M6d.10 is cut in two.**

- **M6d.10a, before D1,** decides what the move needs: what changes which crates leave, their
  names, uwui's colour ([ADR-0296](0296-slime-color-stays-in-the-engine-uwui-its-own-colour-slime-input-leaves-with-uwui.md)),
  uwui's repository's CI and its matrix of platforms, how its ADRs are numbered, and the least of
  its `AGENTS.md` and roadmap that its first session needs.
- **M6d.10b, after M6d.11 and before M6d.12,** plans uwui's first stage in uwui's repository, its
  ADRs written there: everything else that M6d.10 held.

The order of the stage's end becomes: E1, M6d.10a, D1, M6d.11, M6d.10b, M6d.12, alone. Stage 6d's
exit still holds uwui's first stage planned ([`ROADMAP.md`](../ROADMAP.md#stage-6d--uwui-leaves-the-engine)).

**A decision that M6d.10b finds would change what left goes back to the owner** before M6d.10b
goes on, as a change of uwui's repository.

## Alternatives considered

- **M6d.10 whole:** everything decided before the move, but the longest stop of both tracks, and
  the first stage's ADRs born in the engine's repository, consolidated by D1, then moved.
- **The first stage's plan out of Stage 6d,** uwui's repository's first design session: the
  shortest stage, but Stage 6d's exit changed, and the stage's audit, M6d.12, blind to that plan
  and to what it owes the engine ([`uwui-needs.md`](../uwui-needs.md)).

## Consequences

- The move comes sooner by what M6d.10b holds.
- uwui's first stage is planned with its ADRs in uwui's repository from their start.
- The documents that place work "at M6d.10" name M6d.10a or M6d.10b.
- Two milestones in place of one; the cut is kept by one rule: what changes what leaves, its name or
  its CI, is M6d.10a's.

## Validation and reconsideration

Read: M6d.10's deliverables, D1 and M6d.11. Assumed: that no decision of the first stage changes
which crates leave. Proposing another decision is justified if M6d.10b sends a decision back to
M6d.10a's ground more than once.
