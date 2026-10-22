# 0098. The games the engine aims at, and the roadmap they shape

- Status: Accepted; amends [0016](0016-gpu-layer-with-wgpu.md) (native backends after wgpu),
  [0052](0052-amend-roadmap-before-0-1.md) (the editor's UI and the UI of games) and
  [0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md) (the criteria of the
  foundations pass); amended by [0114](0114-hardware-tiers-and-the-gpus-limits.md) (a handheld tier
  below the modest one), [0116](0116-stage-6-in-two-halves.md) (Stage 9c's work ships with 0.11),
  [0131](0131-stage-6b-and-stage-6c.md) (Stage 9c's work ships with 0.12) and
  [0132](0132-the-engines-ui-framework.md) (the UI framework's design and its crates; the
  dev overlay rewritten on it, and egui removed, at Stage 6b)
- Date: 2026-10-19

## Context

The roadmap aims at linear narrative games, open-world games and large online worlds, and grows
one sample game, *Hunna Wood*, a third-person role-playing game in an open world. An engine grown
with one game risks becoming an engine for that game. Before the foundations pass of
[ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md) decides what the editor
will freeze, the owner described the games meant to follow *Hunna Wood*, so that the roadmap is
weighed against them. No order is set between them: the games played alone come first, for
several years, and the online games after them.

The games, as profiles:

1. ***Hunna Wood***: a realistic third-person role-playing game in one open world without loading
   screens, centred on a story whose choices have consequences, with people to meet, demanding
   combat and a horse to ride.
2. **A space opera**: action and story across several hand-made planets, some empty deserts and
   some dense cities, played on foot in first or third person, with cars, motorbikes and
   spaceships; the trip from space to the ground is a loading the player does not notice; the
   player can walk inside their ship while it flies on autopilot; choices can kill characters and
   destroy or close whole places. No procedural generation.
3. **An anime-style narrative role-playing game**: linear, turn-based, toon-shaded everywhere,
   with relationships that change side content and lines of the main story, never its plot, and
   menus with a strong identity of their own.
4. **A hand-drawn 2D metroidvania**: platforming and exploration across a large map of connected
   rooms, with precise controls.
5. **A painted action role-playing game**: a party, real-time combat that slows time while the
   player gives orders, a painterly stylized look, perhaps an open world.
6. **A first-person horror game**: photorealistic, in one small place that repeats and changes at
   each pass, where light and sound make the fear.
7. **A massively multiplayer online role-playing game**: a story told in expansions, an open world
   that grows with each update, about 500 players in one place, instances only for dungeons and a
   few missions, tab-target combat with skills. Personal versions of the world are rare and short,
   or follow the story's progress: past a step, everyone sees the same world.
8. **A competitive first-person shooter** on dedicated servers, at high frame rates on modest
   computers.
9. **Online co-op games** between friends, hosted by a player: physics-driven characters,
   vehicles that players ride and walk in, proximity voice chat.

Across all of them, the owner asked for the best rendering possible in each style, realistic,
anime, painted or hand-drawn: skin, hair, cloth and terrain that react, images that hold still. No
mobile platform is aimed at. Consoles are wanted once there is hardware to test on; the roadmap's
horizon already holds them.

Weighed against these games, the roadmap shows gaps:

- The renderer knows one way to light a surface, physically based
  ([ADR-0035](0035-physically-based-materials.md)), and one tone-mapping curve
  ([ADR-0034](0034-hdr-rendering-and-tone-mapping.md)). Toon, painted and hand-drawn looks need
  lighting, passes and color grading of the game's own.
- No stage holds the AI of characters, dialogue and quests, the state of a story that changes what
  a scene holds, translation and voice, faces, the secondary motion of hair and cloth, vehicles,
  riding, water or weather. Crowds and traffic are in the horizon, outside every stage.
- The physics and audio backends of Stage 7 would be chosen on the first game's needs: bodies,
  colliders and a character controller.
- wgpu 30 offers ray queries, ray tracing pipelines and mesh shaders only as experimental
  features, and one queue per device, so no work on the GPU's compute queues in parallel with its
  drawing. The rendering these games ask for, and consoles, need native graphics APIs at some
  point.
- The foundations pass would write one hardware tier, while the shooter asks for modest computers
  and the realistic games for high-end ones.
- Stage 10b promises several clients on one dedicated server: nothing on lag compensation,
  synchronized physics, voice, games hosted by a player, hundreds of players, or the player as an
  adversary.
- The editor starts on egui, and an engine-owned UI framework was to wait for a measured need
  ([ADR-0052](0052-amend-roadmap-before-0-1.md)). The menus of these games, their text in several
  writing systems and their animated, three-dimensional elements are that need.

The owner decided on 2026-10-19, one question at a time.

## Decision

**The vision lists these profiles, and the ambition that frames them.** Section 1 of
[`docs/ROADMAP.md`](../ROADMAP.md) states the profiles above, without the games that inspire them,
and the aim of the best rendering in each style. Principle 9 stays, and says what the measured
problem is for rendering: each game sets *visual targets*, reference images on its target hardware
at its frame rate, and a technique enters to close the gap between the game's image and its
targets. The engine runs on *hardware tiers*, from modest computers at high frame rates to high-end
ones; a game picks its minimum tier, and each technique of the renderer belongs to a tier. No
mobile platform is aimed at.

**One UI framework of the engine's own, for the editor and the games.** It replaces egui as the
editor's UI. Stage 6 builds what the editor needs (text in several writing systems, layout,
docking, widgets, input routing, focus); Stage 7 adds what games need (animation, transforms,
masks, effects on text, three-dimensional elements). egui stays for the dev overlay, tooling only,
which is removed, as ADR-0052 says, once something does at least as well. The framework's design
(retained, immediate or hybrid) and the crates it builds on are decided at Stage 6, each dependency
asked for then.

**Native GPU backends, at a stage of their own: Stage 9c**, between Stage 9b and Stage 10a,
without a release of its own: what it delivers ships with 0.10. Vulkan and Direct3D 12 behind the
engine-owned API of `slime-gfx`, as the rest of the engine sees it today. The production renderer
of Stage 9a is written on wgpu; a feature that wgpu blocks before Stage 9c goes through wgpu's
access to the API underneath it. Stage 9c decides whether wgpu stays, as a fallback, for debugging
or for macOS, and comes with the decision on the shader language beyond WGSL.

**The foundations pass weighs these criteria**, each in the decision of
[ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md) where it belongs, without
building what the editor does not use:

- the execution model: a game time apart from real time, that slows while the UI and the camera
  run at real speed; background work budgeted so that a hidden loading never drops a frame; a
  simulation that holds the authority apart from what is shown, so that a server can one day run
  it alone;
- the world: a state cheap to snapshot and simulate again, as the prediction of a shooter needs;
  change detection precise enough to send only what changed; identities of entities that can hold
  from one machine to another;
- the frame contract: what a game hands the renderer for an object is not limited to the fields of
  the physically based material, and a frame may hold several views (screens in a cockpit, mirrors);
- the study of open worlds: several worlds, and a state that follows the player from one to the
  other; a whole region loaded during play, then switched to; content of a scene that depends on the
  state of the story (a town whole or destroyed, a character present or dead, a corridor that
  changes), that state belonging to one player or to a step of the story that all share; cells that
  serve streaming and, one day, the split of a world between servers;
- the hardware tiers: a modest minimum at high frame rates, and a high-end tier for the visual
  targets of the realistic games.

**Stage 7** chooses its physics against vehicles (wheels and suspension, motorbikes, free flight),
walking inside a moving vehicle, 2D, characters driven by physics, worlds in double precision and a
simulation that runs the same way twice. It makes *riding* an engine notion: a character attached to
a mount or a seat, with its animations, input and camera. It chooses its audio against sound placed
precisely around the listener's head, reverberation per room, and sounds muffled by walls, on which
proximity voice chat will build.

**Stage 9a** draws many lights by a method that keeps several ways of lighting possible in one frame
(clustered forward is expected; the method is chosen at 9a by measurement); lets a game choose how
each material is lit, add passes of its own, and choose its tone-mapping curve and its color
grading; holds the stability of the image as a measured goal (no shimmering, no trails, no detail
that pops in); lights skin through subsurface scattering; and sets its quality per hardware tier.

**Stage 9b** adds dialogue and quests, with the state of a story they read, in the engine (the rules
of a particular game stay in the game); translation tables and voice; faces through blend shapes,
and lip sync; simple secondary motion of hair, cloth and flesh; motion matching and quadrupeds; 2D
animation; the AI of characters beside navigation. It is split in two halves when planned, if it
needs to be.

**Stage 10a** adds crowds and traffic, with hundreds of characters drawn, terrain that deforms,
hair drawn strand by strand, simulated cloth, water and weather. Procedural generation and physical
destruction stay in the horizon. It is split in two halves when planned.

**Stage 10b** adds lag compensation, synchronized physics, proximity voice chat, games hosted by a
player with relays, hundreds of players in one place and a world split between servers, and extends
the threat model of [ADR-0096](0096-the-files-the-engine-reads.md) to the network. Anti-cheat,
accounts, persistence and the other services of an online game stay in the horizon, a platform apart
from the engine. It is split in two halves when planned.

**A game made by people outside the project is not tried before Stage 11**, whose exit criterion
already asks for one.

## Alternatives considered

- **Profiles without the visual ambition**: shorter, but "a measured problem" would not say what is
  measured for rendering, and a stage could stop at good enough. **Nothing in the repository**: the
  repository is the only memory of the project, and the next reader would not know which games the
  engine is for.
- **The editor on egui, with a framework of the engine's own for games only**, for good or with a
  migration later: Stage 6 smaller and the editor sooner, but two ways to build a UI for years,
  then the editor's UI written again. The large engines build their editors with the UI their games
  use.
- **The native backends as the start of Stage 10a**: one stage fewer, but 10a would hold streaming,
  terrain, vegetation, rendering at large scale and two backends. **Before Stage 9a**: the
  production renderer written for native APIs from the start, but built before any need is
  measured, delaying what wgpu does well today. **A probable step rather than a stage**: the
  rendering asked for and consoles make it certain; only its date is open.
- **The foundations pass without the criteria for networking**, the online games being far: the
  pass would freeze an engine for single players, the trap of grafting networking on later. The
  criteria cost thought now; a simulation apart from what is shown and a world that snapshots also
  serve saves, tests and replaying a bug.
- **Stage 9a with only many lights, image stability and skin**, the look of a game left to 9b's
  material graph: 9a's passes would be written for one look, then written again.
- **Riding left to each game**: three games would write it each.
- **Dialogue and quests left to games or to a plugin**, as other engines do: most of these games
  rest on their story, and each would write the same system without the editor's tools.
- **Strand hair, simulated cloth and deformed terrain left in the horizon**: shorter stages, but the
  details that set the rendering apart would stay optional.
- **Stage 10b only with lag compensation and the threat model**: the co-op games and the
  massively multiplayer one would have no place, and networking would be designed without them.
- **A game made by people outside the project tried after Stage 7 or 8**: a test of how easy the
  engine is to pick up years earlier, but pieces of 2D moved earlier and time spent supporting it.

## Consequences

- Stage 6 grows by the UI framework, and the editor comes later. Upgrading wgpu no longer waits
  for egui once the editor leaves it; the dev overlay still ties the two.
- The foundations pass is longer: its ADRs answer more criteria.
- Stages 7, 9a and 9b grow; 10a and 10b are split in two halves when planned, and 9b if it needs
  to be. Stage 9c is
  added, without a release of its own; the stage numbers that documents and ADRs cite stay right.
- Nothing is built ahead of its stage: each addition enters the stage where a game needs it.
- The order of the stages does not change, except for Stage 9c: the online games come after the
  games played alone, so networking stays at Stage 10b.
- Aiming at both modest and high-end computers makes the known debt of a single GPU tried more
  visible.
