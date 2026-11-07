# 0217. Worlds by density, generated and destroyed

- Status: Accepted; amends [0098](0098-the-games-the-engine-aims-at.md) (destruction and procedural
  generation brought into Stage 10a), [0107](0107-the-execution-model.md) (when background work
  reaches the simulation), [0109](0109-stable-references-and-undo.md) (the identity of what is
  generated), [0112](0112-levels-saves-and-worlds-for-open-worlds.md) (dense content stored by cell)
  and [0208](0208-the-engines-promises-and-systems.md) (the open world's target; generation in play;
  language models for characters)
- Date: 2026-11-07

## Context

[ADR-0208](0208-the-engines-promises-and-systems.md) set the open world's target at 600 km², dense
and made by hand, crossed at 300 km/h without a hitch on the Steam Deck too; tools of placement whose
result is frozen as placements made by hand; nothing generated while the game runs. Destruction and
procedural generation were in the horizon ([ADR-0098](0098-the-games-the-engine-aims-at.md)).

The owner said on 2026-11-07 that 600 km² was a figure chosen at random, and that what is wanted is
density: many characters, as in the market's densest cities, immense buildings, dense and complex
foliage, AI more intelligent, the greatest distance seen. What stands:

- The market's dense worlds measure about 80 to 200 km², by estimates of players and the press:
  GTA V about 80, Red Dead Redemption 2 75 to 127, Cyberpunk 2077 about 100 to 110, Crimson Desert
  about 200. Larger worlds are sparse: Ghost Recon Wildlands about 440 km², Just Cause 3 about 1,000,
  mostly sea, The Crew about 4,900; Microsoft Flight Simulator streams the whole Earth from data.
- A world's area costs the engine little: translations in `f64` reach a planet. Area times density
  costs the content's making, the size on disk and the speed of reading while crossing; what is in
  sight at once costs each frame.
- The world's data does not carry density. Each tree or rock is an entity, its components kept as
  values named by text (`slime_world` records hold `Vec<(String, Value)>`); a scene file holds at
  most 1,048,576 values (`slime_serialize::MAX_VALUES`), which 100,000 entities with transforms
  already exceed.
- An identity is a scene's path, a key, or a number in the order of creation
  ([ADR-0109](0109-stable-references-and-undo.md)): content generated in the background would be
  numbered in the order it arrives, which differs between machines and replays, and a save would
  keep it whole.
- Background work done is applied as it arrives, at the frame's start
  ([ADR-0107](0107-the-execution-model.md)): a load or a generation that ends sooner on one machine
  changes the simulation, which replays, destruction and the network need the same everywhere.
- A small language model fits in 1 to 2 GB, takes GPU time and memory from the image, answers in
  hundreds of milliseconds, and gives results that differ between GPUs.

## Decision

The owner's decisions of 2026-11-07.

**No limit of area; the ambition is density, measured on test worlds.** A district of a city at the
density of the best rival measured, GTA 6 once it is on PC and Cyberpunk 2077 until then:
characters, traffic, towers with their interiors. A forest at the density of foliage of Horizon
Forbidden West and Crimson Desert. A *test world* of 1,000 km² mixing both, made with the tools of
generation below, crossed at 300 km/h without a hitch on every tier, the Deck with less detail and
the same density. A world at the scale of a planet, sparse, to show that no limit holds. Each game
chooses its own size. The 600 km² target is withdrawn.

**Destruction, prepared and computed live,** at Stage 10a, on Stage 7's physics: what an artist cut
beforehand, and what breaks where it is struck. It runs the same everywhere, is kept in saves, is
the same on every machine online, and updates light and the AI's paths. Judged blind against Unreal's
Chaos and the games, with the margins of [ADR-0215](0215-the-engine-measured-against-the-market.md)
on its cost. No world of voxels.

**Procedural generation,** at Stage 10a. At creation: graphs of rules for terrain, rivers, roads,
vegetation, villages and buildings, their result frozen, then changed by hand. In play: small details
seeded on the GPU, the same each time; and, as each game chooses, levels or worlds generated from a
seed, the same on every machine, in replays and online, where the seed is sent rather than the data;
a save keeps the seed and the changes; generation runs in the background within the budgets of
frames. The space opera's planets stay made by hand (ADR-0098).

**Dense content stored densely.** An ADR's rule now, built at its stage: dense content is kept in
compact typed arrays per cell, its edits as differences, and an object becomes an entity only when
something acts on it, a tree felled, a wall destroyed.

**What is generated has an identity:** its generator, the generator's version, its seed and its
region. Its base is generated again when it loads, and the generator's version is kept, so that a
game's update never leaves differences without their target.

**The simulation chooses when work arrives.** What changes the simulation enters only at a step that
the simulation chose; what serves the image alone arrives when it is ready. Stage 7's first work in
the background follows the rule.

**Characters' AI: classical, and language models apart.** The AI of characters is classical and at
the market's best: schedules, perception, memory, factions, behaviours. Language models write, in
tools, variants of lines and reactions, which authors read and freeze. In play, an optional module,
off by default, chosen by each game: language models on the machine, through the engine's own
inference ([ADR-0216](0216-light-geometry-and-the-gpus-floor.md)), within measured budgets, bound
by facts that the authors write, and outside the simulation: they say words and never take a
decision that changes the world, so that replays and the network stay alike. A server only through a
game's own services. Decided at Stage 9b with the characters' AI.

## Alternatives considered

- **600 km² kept:** a simple figure, silent on a city's density, and 600 km² of content to make
  only to measure it. **A minimum area promised to games:** what the engine can do mixed with what
  a game chooses to make.
- **Destruction prepared only:** what the market's largest games do, and nothing for a game of
  survival and building. **Voxels too:** everything destructible, at a cost a realistic world of
  that size cannot pay, and a look imposed.
- **Generation at creation only:** the largest gain, and games that replay or change at each pass
  left to write their own. **In play only:** that gain left aside.
- **The three rules of data left to Stage 10a,** or **only the rule of arrival now:** the tools of
  placement, cooking and saves built on a model to remake, and saves migrated after games ship.
- **No language model in play:** everything controlled, and free conversation, perhaps the
  market's next difference, left out. **Characters driven by language models first:** replays,
  the network and the story's control lost, at a cost per player.

## Consequences

- [`ROADMAP.md`](../ROADMAP.md) moves destruction and generation from the horizon into Stage 10a,
  and replaces the 600 km² target by the test worlds.
- Stage 10a is split: streaming and the data's foundations, cells, dense arrays, generated scenes,
  a budget of spawning per frame, compact identities, saves per region; then destruction,
  generation in play, terrain and vegetation ([ADR-0220](0220-the-roadmap-reshaped-by-the-market.md)).
- The test worlds need the tools of generation first, and their content mostly generated.
- The save's format gains generated scenes before any game ships, so that no shipped save migrates
  for them.
