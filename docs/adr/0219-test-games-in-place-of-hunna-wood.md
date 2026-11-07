# 0219. Test games in place of Hunna Wood

- Status: Accepted; amends [0006](0006-dependency-policy.md) (MPL-2.0 no longer allowed; learned
  models' provenance), [0067](0067-hunna-wood-replaces-nightwatch.md) (*Hunna Wood* no longer the
  sample), [0207](0207-what-slimeengine-is-to-become.md) (the first game sold) and
  [0214](0214-the-engine-under-a-licence-of-its-own-with-a-royalty.md) (the examples and templates
  under MIT-0) and the roadmap's principle 4 (several test games, not one game)
- Date: 2026-11-07

## Context

The roadmap's principle 4 has every stage end with a runnable sample, and from Stage 3c one game
that each stage grows; [ADR-0067](0067-hunna-wood-replaces-nightwatch.md) made it *Hunna Wood*, a
realistic role-playing game in an open world with a story, demanding combat and a horse.
[ADR-0207](0207-what-slimeengine-is-to-become.md) plans a small game of the owner's, sold around
Stage 9b, which proves the chain from the store to updates.

On 2026-11-07 the owner said that *Hunna Wood* is a large project whose part that makes it unique
serves the engine only at a late stage, that it is to be sold one day, and that games made only to
improve the engine would serve it better. What stands:

- *Hunna Wood* is named in 126 files outside `games/`: CI, benchmarks, scripts of measures, the
  editor's Play, the MCP server's tests, documents. Its zone's assets are under CC0-1.0.
- Its code published between 2026-11-05 and the licence notice stays under MIT OR Apache-2.0.
- Users will copy the examples' and templates' code into their own games.
- No dependency uses MPL-2.0, which cargo-deny allows; one that did would oblige every game shipped
  to offer its files' source.
- Styles, menus, faces and the game sold need original art that no plan provides
  ([ADR-0215](0215-the-engine-measured-against-the-market.md)).

## Decision

The owner's decisions of 2026-11-07.

**Small test games, one per profile that leads the stages** (ADR-0207): `games/open-world`, a
realistic slice that takes the present zone and its assets without *Hunna Wood*'s identity, its
story or its characters; `games/arena`, a competitive shooter; `games/metroidvania`, a game in 2D,
after the hand-drawn profile of ADR-0098. Each grows only by what the stage in progress needs. The
test worlds and the scenes rebuilt in Unreal sit beside them.

***Hunna Wood* leaves this repository** for a private one of its own, under the engine's licence,
all rights reserved, its assets still CC0-1.0. It resumes when the engine is ready, as a client from
outside. The move is the first milestone of the stage of measures, before Stage 6c resumes
([ADR-0220](0220-the-roadmap-reshaped-by-the-market.md)): tests, benchmarks and measures move to
`games/open-world`.

**The first game sold is the 2D test game,** around Stage 9b: its code public under MIT-0, a
showcase; its whole content, levels, art, music and identity, in a private repository. Its genre is
decided when it is planned, and the directory renamed if it differs.

**Examples and templates are under MIT-0,** a licence without conditions, made for code that people
copy. So are the test games' code.

**MPL-2.0 leaves the licences allowed to dependencies;** an exception takes an ADR.

**Art.** Libraries under CC0 first, Poly Haven, ambientCG, Kenney, Quaternius; scans that the owner
takes; paid commissions for what cannot be found, first the 2D game sold and one stylised scene of
reference; volunteers with a written consent for captured faces. Agents and tools of generation where
the provenance is clean: agents assemble scenes and make variations; a model is admitted only if its
training data has a clear licence, a rule of ADR-0006; each asset made by a model is marked so in
the assets' credits.

## Alternatives considered

- ***Hunna Wood* kept as the sample:** nothing to move, a large game advancing at the engine's
  pace, the work of a game done too early, and a product to be sold in a public repository.
- **Technical examples only, no game:** the lightest, and nobody using the engine as a studio would.
- **A new small game, private from the start, as the first sold:** a game made for selling, and one
  more project beside the engine and its test games. ***Hunna Wood* as the first sold:** the chain
  of selling unproven for years.
- **Names that evoke,** or **numbered names:** an identity, or nothing said about each game's role.
- **The move now,** inserted after M6d.1c3b, or **at Stage 7:** Stage 6d interrupted for a subject
  apart, or the measures and Stage 6c made on a game about to leave.
- **Examples under the engine's licence:** one regime, and copying an example into a game a legal
  question. **Everything permissive, *Hunna Wood* included:** its code free to sell by anyone.
- **MPL-2.0 kept:** more crates possible, and a duty that a dependency's update could bring unseen.
- **Art by agents and models alone,** or **by libraries, scans and commissions alone:** fastest, of
  uneven quality and provenance; or cleanest, at a cost paid before the engine earns.

## Consequences

- Principle 4 reads: every stage ends with its test games grown and run.
- `deny.toml` no longer allows MPL-2.0.
- The examples hold a licence file of MIT-0 from now; each test game, from its creation.
- Commissions of art are paid before the engine earns; their first two are named above.
