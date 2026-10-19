# 0100. Updates of shipped games

- Status: Accepted; amends [0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)
  (what it leaves for Stage 9b to weigh)
- Date: 2026-10-19

## Context

[ADR-0098](0098-the-games-the-engine-aims-at.md) gave a stage to each need of the games the engine
aims at, but one: updating a game once it has shipped. A game played alone receives fixes after
its release; the massively multiplayer game and the shooter of the roadmap's section 1 live for
years on updates and expansions, and keep their players' data across them.

The roadmap holds little of it. [ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)
leaves archives, patches and downloadable content for Stage 9b to weigh; Stage 11 holds migration
tooling for assets, projects and serialized formats. What exists is the base: a reflected type
reads values written by its earlier versions through its upgrades
([ADR-0080](0080-reflection-and-serialization.md)), and the sample game's save fixture is loaded in
CI. Before its release, the sample's fixture is made again when its identities change, as at 0.4
([ADR-0090](0090-stage-5b-scenes-and-prefabs.md)): no rule says what a shipped game owes its
players' saves.

The owner decided on 2026-10-19.

## Decision

**Stage 9b packages a game for updates.** With its build, cook and package configurations, it
adds archives; patches that download only what changed from one version to the next; and
downloadable content and expansions as packages added to an installed game.

**A rule for every game that ships on the engine**: a save written by a shipped version of a game
loads in all of its later versions. A game keeps a save fixture per shipped version, and CI loads
each of them. The rule starts at a game's first release; before it, a game may make its fixtures
again, as the sample does.

**Stage 10b converts the data that servers keep for players** (characters, progress, inventories)
from one version of a game to the next, with the networking that brings such data. Where that data
is stored stays in the online platform of the horizon.

## Alternatives considered

- **Left for Stage 9b to weigh**, as ADR-0097 leaves it: the only need of the games aimed at
  without a place, and no rule written anywhere about the saves of players.
- **A rule from the engine's 1.0 rather than from each game's release**: the games shipped during
  0.x would break their players' saves at each version of the engine that changes a format.
- **Patches as whole new builds**: the simplest, but each fix downloads the whole game, which a
  large game cannot ask of its players.

## Consequences

- Stage 9b grows by archives, patches and packages of content.
- From its first release, each game carries one save fixture per shipped version, and its CI time
  grows with them.
- A change of the engine that would break a shipped game's saves needs an upgrade that brings them
  forward, even during 0.x.
