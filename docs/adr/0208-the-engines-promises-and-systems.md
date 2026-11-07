# 0208. The engine's promises and systems

- Status: Accepted; amends [0098](0098-the-games-the-engine-aims-at.md) (light without long
  bakes, a reference path tracer, sound, characters' AI, combat, mods, networking and the open
  world's target), [0107](0107-the-execution-model.md) (the simulation parallel and deterministic by
  default) and [0114](0114-hardware-tiers-and-the-gpus-limits.md) (a minimal tier; a mode at 120
  frames a second); with [ADR-0207](0207-what-slimeengine-is-to-become.md) and
  [ADR-0209](0209-platforms-the-editor-and-agents.md), the record of one interview; amended by [0215](0215-the-engine-measured-against-the-market.md) (criteria of physics, sound, AI, animation and the network), [0216](0216-light-geometry-and-the-gpus-floor.md) (one architecture of light, learned models at Stage 9a) and [0217](0217-worlds-by-density-generated-and-destroyed.md) (the world's target by density, generation in play, language models)
- Date: 2026-11-07

## Context

[ADR-0207](0207-what-slimeengine-is-to-become.md) records what the engine is to become, and the
gaps that the survey found in the field. This ADR records what the engine promises, measured, and
what its systems hold beyond [ADR-0098](0098-the-games-the-engine-aims-at.md). A reviewing agent
attacked the promises as the owner first chose them; four were reworded so that they can be kept,
and the owner decided again on those that changed.

## Decision

The owner's decisions of 2026-11-07. Each promise is a budget of the
[quality bar](../ROADMAP.md#4-quality-bar), checked in CI where it can be; the stage that builds a
system measures it.

**No pipeline created during play.** Every pipeline of the GPU that a level uses is listed when the
game is cooked, and created while the level loads, behind its progress, then kept in the driver's
cache: none is created after the level starts. CI replays a game and checks that every pipeline it
asks for is in the cooked list. A game has a budget of pipelines, set with Stage 9b's material
graph, which bounds its permutations, through constants given when a pipeline is made and materials
read by their index. A mod cooks its materials with the tool of mods; they are created when the mod
loads. Shaders reloaded while a game is developed stay outside the rule. Loads in the background,
crossing a world included, make no frame miss its time
([ADR-0114](0114-hardware-tiers-and-the-gpus-limits.md)).

**A change seen within a second.** An asset, a scene or a shader that is changed shows in the
running game and in the editor within 1 s. The game's Rust code is reloaded within seconds, its
state kept. Measured from the edit to the change on screen, on games of 50,000 and of 500,000
lines; how the code is reloaded is weighed at Stage 8 (below).

**One simulation, parallel and deterministic.** A game's simulation runs on every core by default,
on Stage 7's jobs, in a fixed order. Three levels:

1. By default, the same build on the same platform replays a run bit for bit: CI hashes the world
   at each step of a replay.
2. As an option, for strategy games in lockstep and fighting games that replay frames: the same
   across platforms, through the engine's own mathematics and the physics backend's deterministic
   mode, its cost measured against its fastest. The system's functions of mathematics differ
   between Windows and Linux, which is why this is not the default.
3. Work whose order is free runs in the phases that present a frame only, never in the simulation.

The simulation reads time and random numbers from the engine, seeded; a check finds the code of a
simulation that reads the system's clock. Replays, spectating, rewinding while a game is played
([ADR-0209](0209-platforms-the-editor-and-agents.md)) and frames replayed by a fighting game follow
from it.

**Levels that two people merge.** Levels stay text, their entities named by stable identities
([ADR-0080](0080-reflection-and-serialization.md), [ADR-0109](0109-stable-references-and-undo.md)).
An open world keeps one file per cell, and per part placed by hand where that helps. What is placed
in bulk, vegetation and scattered objects, is kept in a binary format with a merge of its own. A
merge tool, which Git calls as a driver, merges the edits of two people or two agents to one level.
Measured: opening one cell, the status of a whole world, merging two edits of one cell.

**Tiers.** A *minimal* tier, below the handheld one, for 2D and light games, measured on the owner's
very modest laptop: once measured, it is named as a class of hardware, its integrated GPU, its
memory and its power, so that anyone can reproduce its budgets; whether its floor admits GPUs
without Vulkan or Direct3D 12, through wgpu's path on OpenGL, is decided then. The handheld tier
stays the floor of realistic 3D. Realistic games offer a mode of quality, the high tier's 3840 ×
2160 at 60 frames a second, and a mode of performance at 120 on the high tier, drawn smaller then
upscaled; its budget, about 5.8 ms on the main thread, is measured once Stage 7's render thread
exists.

**Light without long bakes.** What an artist changes in the light shows at once: no step of hours
anywhere. A ladder, one rung per tier: on the high tier, rays traced by the GPU's hardware; on the
modest tier, tracing in software; on the handheld tier, probes of light updated over several
frames, about 2 ms; on the minimal tier, probes computed while cooking, in seconds, with ambient
occlusion in screen space. Ray tracing is never required. The techniques are chosen at Stages 9a and
10a by measurement, as principle 9 asks.

**The image's ground truth.** A reference path tracer comes with Stage 9a: the image that tests and
visual targets are judged against, the measure of principle 9. A render of film quality, without a
limit of time, for trailers, waits until a game asks for it.

**Upscaling.** The engine's own temporal anti-aliasing and upscaler, and AMD's FSR, open, are
native; NVIDIA's DLSS and Intel's XeSS, whose kits are closed, are optional crates at Stage 9c.
Frames generated between drawn frames stay off by default in competitive games, which they slow to
answer. Running learned models on the GPU, to upscale, denoise, compress textures or animate, is
decided at Stage 9c.

**Physics.** Stage 7 chooses its backend against ADR-0098's criteria and two more: the
deterministic levels above, and an API that holds, or a wrapper inside which its changes stay.

**Sound.** Native and complete: beyond ADR-0098's criteria, a graph of sounds made procedurally, and
music that follows the action. Bridges to FMOD and Wwise, the tools of sound that studios license,
are optional crates.

**Combat.** At Stage 9b, the editor of animation gains a track of combat for each animation: the
frames of a blow's start, its active window and its recovery, the zones that hit, the windows where
it may be cancelled, the stops on impact. The simulation reads it, and replays it the same.

**Characters' AI.** At Stage 9b, beside navigation: perception, seeing and hearing; behaviours
edited as graphs; schedules, a smith who works, eats and sleeps; memory of what the player did, and
factions, saved with the story's state. The rules of a particular game stay in the game.

**Scripts and mods share one sandbox.** Stage 8 adds WebAssembly to the candidates of its scripting,
with one criterion: a game's scripts and its mods run in one sandboxed runtime, reloaded while the
game runs. It weighs too *one language, two targets*: a game's Rust compiled to WebAssembly while it
is developed, reloaded quickly with its state, and natively for the build that ships. Consoles
forbid code made while a game runs; what that leaves is checked under their terms before the choice.

Mods are the engine's, for every game that enables them. Data first: levels, assets, quests. Code
only in the sandbox, with the rights it declares, limits of memory and time, and a failing mod
stopped alone. A mod's manifest is signed, and so are plugins. The editor is the modders' tool, as
it is the studio's.

**Networking, at Stage 10b.** The server holds the authority, as ADR-0098 has it, and finds cheating
on its side; Easy Anti-Cheat and BattlEye, which run under Linux and Proton, are optional crates; no
anti-cheat in the system's kernel, ever, which would shut out Linux and the Steam Deck. Promised and
measured:

- the delay that a player feels, from an input to its effect seen, a shot that hits: budgeted and
  measured, not only the server's rate; a model right on paper was judged by its feel;
- a simulator of networks, in the editor and in CI: latency, loss and jitter, agents playing several
  clients under a bad network;
- a game hosted by a player goes on with another host when the host leaves;
- players of different stores play together; a player's identity belongs to no store.

**The open world, at Stage 10a.** A world of 600 km², dense and made by hand, crossed at 300 km/h on
the ground and in a ship flying low, without a hitch or a detail that shows late, on the Steam Deck
too. No limit holds beyond it: translations in `f64` allow larger worlds, sparser. Tools of
placement come with Stage 10a: they spread trees, rocks and objects by rules, and their result is
frozen and saved as placements made by hand, which the editor then changes; nothing is generated
when the game runs. Cooking has a budget of time per km², and cooks on several machines later.

The reviewing agent judged this target out of reach on the handheld tier, and the owner kept it.
Its estimates, not measurements: 300 km/h is 83 m/s; at full detail across a front about a
kilometre wide, about 0.08 km² of new ground each second; at about 1 GB per km² of dense hand-made
world, about 80 MB/s read without a pause, the rate of a microSD card, where many Steam Deck players
install games; about 600 GB installed, against Steam Decks of 64 GB to 1 TB. The densest worlds
shipped, by public estimates, hold from a few tens to about a hundred km² of land; those that reach
hundreds or thousands are far sparser. Stage 10a measures it on a world of test of 600
km², of the density of one km² made by hand: megabytes per km² on disk, the rate read while
crossing, what stays resident within the handheld tier's 4 GB, and how far a detail is off when its
level of detail changes. A budget missed is debt, as every budget.

**Tests over content.** From Stage 7, replays drive recorded runs through the sample at each push,
and a change that breaks a run or slows it is found by its commit.

**Accessibility.** The Xbox Accessibility Guidelines are the list of what the engine gives, native
and checked, beyond Stage 7's: menus read aloud, the sounds captioned with their direction, the
game's sound lowered while a screen reader speaks, help to aim, sizes of text, controls with one
hand. A game chooses what it turns on.

**Languages of games.** Translation through Fluent, which uwui brings, with plurals and the formats
of dates and numbers; voices per language; a fake language of test in CI, longer and accented, which
finds untranslated and cut texts; text from right to left.

## Alternatives considered

- **Shaders compiled while played, cached afterwards**, as the largest engines do: no budget of
  pipelines, but the hitch that players complain of most, reduced and never removed. **"Zero
  shaders compiled", as the owner first chose it**: right as a goal, wrong once drivers, mods and a
  material graph enter; reworded as no pipeline created after a level starts.
- **Deterministic across platforms by default**: one rule, but the engine's own mathematics in every
  game, and slower physics, for the two kinds of game that need it.
- **One simulation thread**, as the most used engine of large studios has: simpler code in games,
  but the limit that engine now means to remove.
- **Binary levels with locks**: no merge to write, but teams that lock files, and agents that cannot
  read what they change.
- **Steam Deck as the floor of every game**: one tier less, but 2D and light games shut out of
  machines they would run on.
- **Light dynamic on every tier**, as the owner first chose: one way, but a blurred image on the
  Steam Deck, and no lit 3D on the minimal tier. **Light baked on the lower tiers**: fine and cheap
  on small machines, but hours of baking, and no day and night nor places destroyed.
- **The vendors' upscalers alone**: less work, but nothing good on a GPU without its own, and kits
  that are closed.
- **A render of film quality as an option from Stage 9b**, as the owner first chose: trailers
  without another tool, but a path of rendering for no measured problem; the reference path tracer
  serves principle 9 first.
- **Sound on FMOD or Wwise first**: tools that sound designers know, but closed licences at the core
  of a free engine, out of the agents' reach.
- **Mods left to each game, or none**: nothing to build, but each game rewrites them, often without
  safety, and role-playing games live long through their mods. **Native code in mods**: anything
  allowed, and the malware that turned mods off in several games in 2026.
- **Scripting left wholly to Stage 8**: nothing imposed, but the risk of two runtimes to secure.
- **An anti-cheat in the kernel allowed**: the strongest protection, and games that run on neither
  Linux nor the Steam Deck.
- **The open world crossed at a horse's pace**, or **400 km²**, which the owner raised to 600: easier
  to keep, but racing and the space opera redone later.
- **A target split in two**, the engine measured on 600 km² and the game as large as its team can
  fill, which the reviewing agent proposed: kept as how the target is measured; the owner kept the
  target itself.
- **Characters' schedules, memory and factions left to games**: lighter, but two of the leading
  profiles' games rewrite them.
- **The minimum list of Stage 7 for games' accessibility**: less to build, and each game rewriting
  the rest.

## Consequences

- The quality bar gains budgets: pipelines created after a level starts, zero, in CI; a change seen
  within 1 s; a replay identical bit for bit, in CI; the merge of two edits of a level; the minimal
  tier; the mode at 120; the editor on the modest tier; the light's rung per tier; the felt delay of
  a networked game; the open world's figures. Each is measured from the stage that builds it; until
  then it is written as a target.
- Stage 7 gains the deterministic simulation, its check by hash, the replays of content and the
  physics' two criteria; Stage 8 the sandbox shared by scripts and mods, and mods; Stage 9a the
  ladder of light and the reference path tracer; Stage 9b the budget of pipelines, the track of
  combat and characters' AI; Stage 9c learned models on the GPU and the vendors' upscalers; Stage
  10a the open world's target, placement and the cooking's budget; Stage 10b the four promises of
  the network and the engine's own relay and server.
- Writing a game's simulation asks more of its authors: an order fixed, no system clock, the
  engine's random numbers.
- The open world's target may stay debt on the handheld tier; its measures say so plainly.
- The sample's readers of levels change at Stage 10a, one file per cell; the user's levels are
  migrated, as ever.
