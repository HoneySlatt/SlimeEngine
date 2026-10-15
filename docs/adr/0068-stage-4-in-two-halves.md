# 0068. Stage 4 in two halves, and audio import at Stage 7

- Status: Accepted
- Date: 2026-10-15

## Context

The roadmap's Stage 4, the asset pipeline, released as 0.2, holds two kinds of work:

- turning source files into data that a game loads: stable asset IDs, metadata and dependencies,
  importers for glTF, images and audio, tangents computed with MikkTSpace, a cooked cache rebuilt
  incrementally;
- making loaded assets live: loading them on a task pool, from threads that create GPU objects,
  materials whose values change in place, hot reload, and recovering from a lost GPU device by
  creating the GPU objects again from their sources.

Planned with the game that the stage grows, *Hunna Wood*
([ADR-0067](0067-hunna-wood-replaces-nightwatch.md)), the stage came to about fourteen milestones.
The second kind of work builds on the first: what loads in the background, reloads or is created
again is what the first has made loadable by ID.

Nothing in the engine plays sound before Stage 7: an audio importer at Stage 4 would cook files
that nothing reads, which no test or game could check.

Stage 1's roadmap entry sends configuration files and a virtual filesystem to Stage 4, which lists
neither.

## Decision

The roadmap's Stage 4 is split in two halves, as Stages 3 and 9 were
([ADR-0052](0052-amend-roadmap-before-0-1.md)): a reference to Stage 4 means both.

- **Stage 4a, Cooked assets, released as 0.2**: stable asset IDs, metadata beside the sources,
  dependencies; importers for glTF and images; tangents computed with MikkTSpace; textures
  compressed for the GPU; the cooked cache, rebuilt incrementally. A game loads its assets when it
  starts, on the thread that asks. Exit: *Hunna Wood* loads its zone from cooked assets and is
  played by day and by night, on Linux and on Windows; the temporary glTF loader is deleted; moving
  a source file breaks no reference.
- **Stage 4b, Live assets, released with 0.3**: loading on a basic task pool, from threads that
  create GPU objects through the `Gpu` they share; materials whose values change in place; hot
  reload of textures, models, materials and shaders; recovering from a lost GPU device. Exit: a
  texture and a shader hot-reload in the running game; the zone loads while frames go on; the game
  goes on after a lost device. Stage 4b is planned with the owner at the end of Stage 4a, and has
  no release of its own, as Stages 3c and 3d.
- **Audio import moves to Stage 7**, with the audio that plays what it imports.
- **Configuration files and a virtual filesystem are not part of Stage 4.** A game reads its cooked
  data from a directory; each comes when a stage needs it.

The owner chose this on 2026-10-15.

## Alternatives considered

- **One stage of about fourteen milestones, released as 0.2 at its end.** One plan, but no release
  until the second kind of work, the riskier, is done, and a longer time without one.
- **Two halves, with 0.2 after both.** The same delay, with the halves only as a way to plan.
- **WAV import now**: little code, but an importer whose output nothing plays.

## Consequences

- 0.2 comes sooner, without hot reload, background loading or recovery: until Stage 4b, a game
  loads at startup, and a lost device still stops the run (ADR-0022).
- The roadmap's Stage 4 becomes 4a and 4b, its release map gives 0.2 to Stage 4a, and Stage 7
  gains the audio importer.
- Stage 4b's work comes before Stage 5's, and its release waits for Stage 5's.
