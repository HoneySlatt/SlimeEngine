# 0067. Hunna Wood replaces Nightwatch as the sample game

- Status: Accepted; amends [0052](0052-amend-roadmap-before-0-1.md) (one game grown by every
  stage) and [0055](0055-games-in-the-workspace.md) (the first game)
- Date: 2026-10-15

## Context

From Stage 3c on, the sample that every stage ends with is one game, grown by each stage
([ADR-0052](0052-amend-roadmap-before-0-1.md)), in `games/`
([ADR-0055](0055-games-in-the-workspace.md)). That game is *Nightwatch*, a small stealth game in a
courtyard at night, made of boxes, spheres and squares built in code, which Stages 3c and 3d grew.
What it asked of the next stages is in
[ADR-0057](0057-what-the-first-game-asks-of-the-next-stages.md).

Stage 4's exit asks the game to load its scenes from cooked assets. Loaded from cooked data,
*Nightwatch*'s courtyard would test an asset pipeline little: a few shapes, no imported model, few
textures. The engine's first target is narrative games, then open worlds (`docs/ROADMAP.md`,
section 1), which a stealth game in one courtyard does not approach.

When Stage 4 was planned, the owner asked that the sample become the game they mean to make with
the engine: a role-playing game seen in the third person, in a dark-fantasy world. Its direction,
as the owner set it on 2026-10-15, for the long term:

- demanding combat, mostly with melee weapons, magic a full part of it, and weapons and spells
  varied enough that characters built around them play very differently;
- characteristics that grow with use, physical ones and social ones, such as speech or kindness;
- a written hero with a name and a voice, a dense world and its past, choices whose consequences
  weigh on the story, relationships that can go wrong;
- a dark-fantasy look: monumental architecture and nature, landscapes composed like paintings;
- one seamless open world, with graphics close to those of the best open worlds of its time.

The owner knows that the game starts much smaller than this.

## Decision

- **The sample game becomes *Hunna Wood*,** from Stage 4a on. It lives in `games/hunnawood/`; its
  package and its program are named `hunnawood`, its title *Hunna Wood*. Like *Nightwatch*, it uses
  the engine's public API only (ADR-0055).
- **What Stage 4a makes of it**: one zone of its starting region, a forest path explored in the
  third person, by day and by night, with no combat, dialogue, physics, animation or sound, which
  the engine does not have yet. `docs/PLAN.md` lists what the zone holds and what the player does
  there.
- ***Nightwatch* stays until *Hunna Wood* is played by day**, then is deleted (M4a.7). It is never
  released: 0.2 ships *Hunna Wood*. Until then, both games build and run in every check.
- **The direction above is direction, not a commitment.** Each stage still builds what its
  milestones need. What the game asks of the engine is recorded as it comes, as *Nightwatch*'s
  findings were (ADR-0057), and proposed to the owner with the stage it belongs to. The game may
  ask some systems sooner than the roadmap orders them, such as animation or physics; each such
  change is proposed then, with an ADR.
- **The repository describes the game as far as it is made**: its README describes what can be
  played. The story and the world beyond what the stages need are kept outside the repository.
- **ADR-0057's findings stand.** *Hunna Wood* takes over the workarounds *Nightwatch* keeps until
  the stages that remove them: the keys it holds, its own collisions and lines of sight, the
  previous transforms of what moves, a material per state.

This amends ADR-0052, whose sample was one game grown by every stage: a stage may now replace the
game, with an ADR. It amends ADR-0055, whose first game was *Nightwatch*. The roadmap's principle 4,
Stage 3c and its release map follow.

## Alternatives considered

- **Keeping *Nightwatch* and growing it.** No new game to start, but a courtyard of boxes asks
  little of an asset pipeline and nothing of the engine's direction, and the owner wants to make
  the role-playing game.
- **Starting *Hunna Wood* beside *Nightwatch*, frozen but kept and tested.** *Nightwatch* would
  keep testing what it tests, at the cost of a second game to follow through every change of the
  API, which no stage grows.
- **Keeping *Nightwatch* for Stage 4 and starting *Hunna Wood* at Stage 5.** Stage 4's formats would
  be designed for a courtyard of boxes, and the game would meet them already made.
- **Another kind of role-playing game**: seen from above; turn-based; in the first person. The
  owner chose the third person.

## Consequences

- *Nightwatch*'s code, README, tests and `CHANGELOG.md` entry are removed at M4a.7, and the plan
  records the last commit that holds it.
- The sample needs content that code does not make: models and textures from free libraries, and
  what Blender makes from them ([ADR-0070](0070-the-sample-games-source-assets.md)). That takes time
  that code does not, which the plan bounds with budgets.
- The game shows the renderer's gaps more than a courtyard did: no fog, ambient occlusion or
  transparency, no culling or instancing, foliage cut by masks whose edges stay aliased, point
  lights that cast no shadow, the sky's light that reaches under trees unoccluded. The known debt in
  `docs/PLAN.md` lists them; the game records what each costs it.
