# 0215. The engine measured against the market

- Status: Accepted; amends [0098](0098-the-games-the-engine-aims-at.md) (faces, secondary motion
  and crowds raised), [0114](0114-hardware-tiers-and-the-gpus-limits.md) (the Steam Deck at 40
  frames a second; a mode at 120 on the modest tier; a game's memory halved),
  [0175](0175-the-quality-bar.md) (rivals measured, named and beaten by a margin; input timed to the
  photon), [0176](0176-fuzzing-and-crash-reports.md) (hours of use counted; reports sent by choice),
  [0189](0189-first-measures-of-time.md) (latency measured to the photon too),
  [0207](0207-what-slimeengine-is-to-become.md) (games named as benches; reports sent by the user's
  choice), [0208](0208-the-engines-promises-and-systems.md) (criteria of physics, sound, characters'
  AI, animation and the network), [0209](0209-platforms-the-editor-and-agents.md) (the editor and
  agents measured against rivals) and [0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md)
  (uwui measured as the engine is; no look imposed on applications); amended by [0226](0226-an-application-started-in-one-call-each-root-given-its-look.md) (blank is Plain, a variant of the core's look)
- Date: 2026-11-07

## Context

[ADR-0207](0207-what-slimeengine-is-to-become.md) judges the engine as a whole against Unreal,
Unity and Godot, and better as a whole. The quality bar of [`ROADMAP.md`](../ROADMAP.md#4-quality-bar)
holds absolute budgets: no rival is measured, no lead is asked for, and each game's image is
judged against its own targets alone. On 2026-11-07 the owner asked that the engine beat the
market on every point, image, effects, light, smoothness, resources, AI, sound and physics, and
judged the bar too low. What a review of the bar found:

- In image, the rivals are Unreal 5 and engines held by their studios: RAGE, Decima, BlackSpace,
  REDengine. Only Unreal can be used to build a scene; the others are seen through their games.
- Frames: 99 % on time and none over twice the frame let one frame in a hundred stutter. A late
  frame on a display of fixed rate shows as a repeated one.
- The frame rates are a good game's of today: 30 a second on the Steam Deck. Whether 4K is drawn
  natively or upscaled is not said.
- A game's memory is nearly the whole machine, 8, 12 or 24 GB. Nothing bounds the size installed,
  the Deck's battery or the processor's load. A game's latency is two frames, measured inside the
  engine, which no game of the market can be compared with.
- Physics, sound, characters' AI, animation and the network have systems listed and no bar. Faces
  stop at blend shapes, secondary motion at "simple", crowds at hundreds.
- Stylised looks, menus and the sameness that games of one engine share have no bar.
- The editor's figures are far ahead, and absolute: no rival opens the same level.
- Agents drive the engine through its MCP server; nothing compares how well they do it with the
  rivals' tools for agents, and models know Unreal and Unity from their training, not this engine.

## Decision

The owner's decisions of 2026-11-07. Each figure missed is debt until met; each is measured as the
stage that it touches is built, and the rivals' figures before it.

**Rivals, measured two ways.** Games on PC measured from outside, on the owner's machine and the
Steam Deck: frames and their pacing, hitches while crossing the world, memory and video memory,
power, the processor's load, the time to control, loads, the size installed and the image's
stability. The best game on each criterion is the bar: Red Dead Redemption 2, Cyberpunk 2077,
Horizon Forbidden West, Crimson Desert and Death Stranding 2 to begin; GTA 6 once it is on PC.
Unreal 5 on scenes rebuilt alike with the engine's own assets: the image against the reference
path tracer, and every cost, at equal content. Each measure on the owner's games and account waits
for the owner's consent, as M6d.0's did.

**Games named as benches only.** A game or an engine is named in the quality bar's tables and in
[`measurements.md`](../measurements.md), with its version, settings and date, as a bench. Never as
an inspiration: nothing about a game of the owner's names another game.

**The lead, by family.** Against games, on what the player lives, hitches' time, the time to
control, loads and latency: two and a half times better than the best; an image steadier than the
best. Against Unreal on the same scenes: four times less processor, memory, start and build size;
1.75 times less GPU time and power at equal image; at equal GPU time, an image closer to the path
tracer. Beating each line stays the least.

**Frames and latency.** 99.9 % of frames on time, none over one and a half times the frame, none
missed while crossing the world fast or while loading. Latency at the 99th percentile: one and a
half frames for a realistic game, one for a competitive shooter. A light sensor on the screen,
which the owner buys, times input to photon, so that games of the market compare from outside;
the trace inside the engine stays.

**Frame rates.** A realistic game at 40 frames a second on the Steam Deck, 25 ms, 30 a fallback
that a game may choose; a mode of performance at 120 on the modest tier too, at 1080p; the high
tier keeps 3840 × 2160 at 60 and 120. The resolution shown counts; the resolution drawn is free,
the image judged by measure, against the path tracer and for its stability.

**Resources.** A game's memory halved: 6 GB on the Deck, 6 GB on the modest tier, 12 GB on the
high one; video memory for assets unchanged, as the image needs it. The size installed per km² of
world made by hand at most half the best game's. The realistic test game at 40 frames a second on
the Deck lasts at least one and a half times as long on its battery as the best game measured at
its settings for the Deck. On average, at most half of the tier's cores at the target rate.

**Physics, sound, characters' AI, animation and the network,** with criteria now and figures set at
the start of the stage that builds each, against games and against Unreal's Chaos, MetaSounds, AI,
animation and replication, Iris, on the same scenes, with the margins above:

- physics: bodies simulated within a frame, stacks that hold still, nothing that passes through a
  wall at 300 km/h, the cost of each vehicle, cloth and hair;
- sound: voices at once and their processor, the time from an action to its sound, no dropout,
  placement and muffling judged in blind listening;
- characters' AI: characters fully alive within a budget, the cost of each, a path's query, the
  time to react; believability judged in sessions of play by the owner and by agents;
- animation: feet that slide, in centimetres a step; input to a change of pose; characters
  animated within a budget and the cost of each; faces and reactions judged blind. Faces go beyond
  blend shapes, with learned correctives; reactions are physical, blended with animation; hair and
  cloth are simulated; crowds are of thousands;
- the network: a shooter's felt delay, the peeker's advantage and the server's rate, against
  Valorant and Counter-Strike 2; a fighting game's frames replayed with no visible glitch at 100 ms
  of round trip; 500 players in one place; the time for another player to take over a game whose
  host left; the server's processor and bandwidth per player four times less than Iris's.

**Characters, in three levels of life.** Visible: at least two and a half times the best game's
density in its busiest street, at the same frame budget. Alive: each character near the player has
a schedule, perception and a memory of the player's acts; no hollow extra within a distance set at
Stage 9b. Simulated out of sight, lighter. The Deck keeps the density, with less detail. Figures at
Stage 10a.

**No visible limit to the distance seen.** Everything shows at its true distance to the horizon,
from any height: mountains at 50 km and more, a planet from its sky, a city's lights at night, far
traffic that still moves. Shadows and light hold far, simpler. No detail appears in sight, judged
blind and measured while crossing. The Deck sees as far, with less detail.

**Realism, subject by subject:** skin, eyes, faces, hair, cloth, vegetation, water, sky and clouds,
terrain. Each against the path tracer, against photographs taken in known conditions, and against
the rivals' captures, judged blind, with its cost per tier. Faces: a performance captured with a
plain camera, a face driven by its voice alone in each language, with its emotions; whole eyes,
their refraction, wetness, blinks and small motions; wrinkles that come with an expression.

**Stylised games, of the first rank.** A scene for each style, made with the engine's own art:
anime with cel shading, a cartoon of the 1930s, painted, drawn by hand in 2D; never copying a
game's look. Tools that the artist drives: ramps of light, face shadows drawn, outlines steady at
every distance, characters lit apart from the world, animation in steps with smears, a line that
boils and grain, shadows and indirect light stylised. Judged blind against captures of Genshin
Impact, Wuthering Waves and Mouse: P.I. for Hire, at the same cost per tier, light on the Deck and
the minimal tier. The tools at Stage 9a; animation in steps at Stage 9b.

**Menus.** A menu of reference, with the engine's own art and a strong identity: every input
answered in the frame itself; every transition interrupted without a jump; no hitch as any screen
opens, all preloaded; sound on the motion within a frame; 0.5 ms of GPU at 3840 × 2160. A timeline
animates menus without code, for designers and agents, a general need of uwui's. Judged blind
against captures of Persona 5 Royal and Metaphor: ReFantazio. Foundations at Stage 7, the timeline
at Stage 9b with the sequencer. No library of ready-made effects of menus, which would make menus
alike.

**No look of the engine.** Nothing is on by default: each effect after the frame and the curve of
tones are chosen in a game's *look file*. A new game starts neutral, its curve Khronos PBR Neutral
written in its look file, or from one of several looks very unlike each other. The model of light
is the game's, as easy to write as a material, without changing the engine. Faces and characters
come from people captured or from artists, never from one library ready to use; the store pushes no
look. A *test of signature*: judges shown the engine's looks among other engines' games guess which
share an engine; the engine's are recognised no more often than chance, at the end of each stage
that touches rendering. uwui follows the same rule: its core's look is sober, a start; `uwui new`
asks which look, its own, Glass, Material 3 Expressive, Pro or blank; nothing is chosen in silence;
a test of signature checks applications.

**Blind tests by people.** People compare captures of the engine's and Unreal's on the same scenes,
or of the engine's and the games', without knowing which is which, at the end of each stage that
touches rendering: the owner and relatives first, the community after 0.14.

**Creation, measured.** A *creation bench*: fixed tasks, a km² of dense forest with a village, a
character imported and animated, a lit interior, a short cutscene, a quest with its dialogue, done
in the engine and in Unreal by the same person and agents. It measures the human time, the agents'
tokens and the machine's waits, imports, cooking, shaders and light, the quality judged blind: four
times less waiting, two and a half times less human time, the bias of human time written beside
each figure.

**Compiling.** On the Ryzen 7 5800X, at each milestone: a change of a game's code to the game
running again, 2 s in a debug build; reloaded hot, its state kept, under 1 s; the editor built
again after a change of an engine crate, 10 s; the whole engine from nothing, 3 min; a game's build
for shipping, 10 min. Against Unreal, Unity and Godot, four times less waiting than the best.

**The editor.** The same level opened in Unreal, Unity, Godot and Blender: start, level opened,
memory, the 3D view's smoothness, input to present, rest; four times better than the best on each
line. At least 99.9 % of hours of use without a crash, and no work lost after a crash, checked in
CI. An open world's cell opened and crossed, its figures at Stage 10a against Unreal's World
Partition.

**Agents.** An *agent bench* of the engine's tasks: a system of a game written, a level built from
a brief, a bug fixed from a crash report, a scene brought within its budget; given to the same
agents in the engine, Unreal, Unity and Godot, each with its own tools for agents; success, tries,
tokens and time, at each release; the goal, provisional, at least the best's success on fewer
tokens. Documentation written for agents ships with the engine; each new game gets an `AGENTS.md`.
*Tester agents* play the test games end to end through the MCP server at the end of each stage, and
report what blocks, crashes or misses a budget.

**Crashes counted.** Reports stay on the machine; testers send theirs by hand. From 0.14, a user may
send crash reports and hours of use, anonymous, seeing what leaves and able to stop it. The editor
asks at its first start, explaining what the data serves and showing yes as the choice recommended,
and the dialog after a crash offers to send that report. Nothing is ticked in advance, and no is as
easy as yes.

**uwui measured as the engine is:** the light sensor, blind tests by people, budgets of compiling
and hours without a crash apply to applications on uwui, in its quality bar.

## Alternatives considered

- **Unreal alone**, or **games alone**: one exact comparison, without the studios' engines that
  lead open worlds; or games whose art cannot be told from their technique.
- **Games described, not named:** nothing told, and no figure that anyone could measure again.
- **One factor everywhere,** three times: out of reach on GPU time at equal image against Unreal, a
  debt that no work would settle. **No margin:** a lead of 1 % passes.
- **Every frame on time, and one frame of latency for every game:** one hitch of the system, outside
  the engine, fails a measure; effects that need a deeper pipeline paid for no felt gain.
- **60 frames a second on the Deck:** a realistic image the Deck cannot hold.
- **Memory quartered:** content or image cut in a dense open world.
- **Figures now for physics, sound, AI, animation and the network:** guessed, years before their
  stages.
- **A fixed count of characters,** or **a fixed distance seen:** chosen at random, as the world's
  size had been.
- **A renderer apart for stylised games:** against ADR-0052, and without the dynamic light.
- **A library of effects for menus:** faster starts, menus alike.
- **A curve of tones at none,** or **no default at all:** a first image that clips, or a question a
  beginner cannot answer.
- **The editor's figures left absolute:** a lead claimed, not shown.
- **Crash reports sent by default:** most data, against the rule that nothing leaves of its own
  accord.

## Consequences

- [`ROADMAP.md`](../ROADMAP.md#4-quality-bar) holds every figure above, and names the rivals; uwui's
  quality bar takes its part.
- Tools come before the stages that are judged by them: a game measured from outside, the light
  sensor's bench, a tool of blind tests, the rivals' editors and compiling, the scenes rebuilt in
  Unreal ([ADR-0220](0220-the-roadmap-reshaped-by-the-market.md)).
- The editor's crash reports grow beyond a panic's hook: the hours of a session, crashes that are
  not panics, a journal that survives a torn write, checked by a test that kills the editor.
- Art of the engine's own is needed for the styles, the menus and the faces
  ([ADR-0219](0219-test-games-in-place-of-hunna-wood.md)).
- The owner's consent is asked before any measure on the owner's games and account.
