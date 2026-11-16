# 0287. The simulation's contract designed at Stage 7, its questions written now

- Status: Accepted
- Date: 2026-11-16

## Context

[ADR-0208](0208-the-engines-promises-and-systems.md) promises a simulation that runs on every core
and replays bit for bit on one build and one platform, its time and randomness given by the
engine. On 2026-11-16:

- `Simulation::fixed_step` takes `&mut World`, as do 119 parameters of the engine and its games;
- `World::new_id` draws identities from the system's randomness (`std::hash::RandomState`);
- queries walk in no particular order, which changes as components come and go;
- the engine gives no randomness of its own.

Stage 7 builds the execution, the determinism and the replays
([`ROADMAP.md`](../ROADMAP.md#stage-7--runtime-systems--08)). No stage of the engine runs before
uwui's 1.0 ([ADR-0277](0277-the-engine-ported-to-uwui-at-the-end-of-each-of-uwuis-stages.md)):
no system of a game is written until then.

## Decision

The owner's decision of 2026-11-16.

**The simulation's contract is designed by Stage 7's design session,** on a trial of the parallel
run on one case, before the stage's systems. Its questions are written now in Stage 7's entry of
the roadmap, so that none is lost: how threads reach the world beside `fixed_step(&mut World)`; the
queue of deferred changes; randomness seeded by the engine, identities included; an order of
iteration that queries promise; the rule of floating-point arithmetic; a typed copy of the world,
for replays and rewinding; a host without a window or a GPU, for the replay check in CI.

**From now, a simulation reads time from its context alone, never a clock, and no randomness of
the system,** as the rustdoc of `slime_app::Simulation` says. Code written before this rule is
found and changed by Stage 7's design session.

## Alternatives considered

- **The contract decided now:** settled early, but decided without a trial, two stages before it
  is needed.
- **The identities seeded now, the rest at Stage 7:** a first step, but one part of what is designed
  as a whole.

## Consequences

- Nothing changes in the code before Stage 7 but the rule.
- Stage 7's design session starts from the list, the trial and the code as it is then.

## Validation and reconsideration

Read: `fixed_step`'s signature and the parameters that take the world; `World::new_id`; queries'
order. Assumed: that no system of a game is written before Stage 7.

Proposing another decision is justified if a stage before Stage 7 writes systems of a game.
