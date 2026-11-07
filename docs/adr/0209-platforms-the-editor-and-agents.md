# 0209. Platforms, the editor and agents

- Status: Accepted; amends [0098](0098-the-games-the-engine-aims-at.md) (consoles through private
  layers; the Steam Deck a platform of the first rank; virtual reality, mobile and web games in the
  horizon), [0099](0099-ai-agents-drive-the-engine.md) (rights given to each agent; a paired
  machine on the local network), [0122](0122-the-development-channel.md) (the channel opened to a
  paired machine) and [0190](0190-uwuis-requirements.md) (games' interfaces on uwui's core with a
  layer of the engine's); with [ADR-0207](0207-what-slimeengine-is-to-become.md) and
  [ADR-0208](0208-the-engines-promises-and-systems.md), the record of one interview
- Date: 2026-11-07

## Context

[ADR-0207](0207-what-slimeengine-is-to-become.md) records what the engine is to become. This ADR
records where it runs, what its editor makes and leaves to other tools, and what agents gain in it.
The editor's design until Stage 6c's end is [ADR-0155](0155-the-editors-design.md)'s, which this
ADR leaves as it is: what follows comes at Stage 7 and after.

## Decision

The owner's decisions of 2026-11-07.

**Consoles, through private layers.** When the owner has consoles and their kits, each console is a
layer of its own, in a private repository, given to the studios that its maker approves, as open
engines that reach consoles do. The public engine holds no code under a console's agreement, and no
choice that would keep one out: its platform layer stays narrow, its core builds without the
system's services. The layers are also a source of income
([ADR-0207](0207-what-slimeengine-is-to-become.md)).

**The Steam Deck, a platform of the first rank.** Not Linux by chance: before each game's release,
the Steam Deck's criteria of verification are checked, on the owner's Steam Deck: controls and their
glyphs, sizes of text, sleep and wake, the keyboard on screen. Steam's integration, an optional crate,
gives Steam Input with the glyphs of each controller, the keyboard on screen and saves in Steam's
cloud; GOG's and Epic's come after it, each a crate of its own.

**Virtual reality, mobile and the web, in the horizon.** Headsets through OpenXR, after Stage 9c,
when a game asks; until then, nothing in the renderer may stop two views in stereo at a high rate,
which views apart from the renderer already allow
([ADR-0126](0126-views-apart-from-the-renderer.md)). The owner's games stay without mobile; for the
engine's other users, games on Android and on the web are studied after 1.0, once uwui runs there
and a user asks.

**A paired machine on the local network.** In development builds only, the development channel and
the MCP server open to a machine that the user pairs explicitly, by a code: encrypted and
authenticated, on the local network, so that the Steam Deck, and one day a console, is profiled and
driven from the user's computer. Shipped games never hold it. Elsewhere,
[ADR-0099](0099-ai-agents-drive-the-engine.md)'s rule stays: the user's own machine only. The
threat model of [ADR-0096](0096-the-files-the-engine-reads.md) is extended to the pairing when it is
built.

**What agents gain.** Beyond ADR-0099, the engine's MCP server gives:

- rights for each agent, declared and checked: reading only, a folder, never the network; ADR-0099
  set an agent's rights to those of the person who runs it, which become the most an agent may have;
- a preview before applying: an agent shows what its change would make of the level and of the
  image, then applies it;
- tools found by families, on demand, so that their list never fills the agent's context;
- a game played without a window, from a seed, and played again exactly the same, through the
  deterministic simulation ([ADR-0208](0208-the-engines-promises-and-systems.md)).

Each comes with the stage whose system it serves, the agents' continuous track.

**Agents in the editor.** A panel of the editor's own shows what agents do: who changed what, the
previews waiting for the user's yes, the rights of each, and buttons to accept, refuse or undo. An
assistant within the editor, an optional crate at Stage 11, talks with the user and acts as any agent
does, through the same rights and previews, with no provider imposed: the user chooses a service or
a local model, with their own key. It sends nothing until the user sets it up, so that the engine
still sends nothing of its own accord.

**What the editor makes.** The world: levels, terrain, vegetation, light, gameplay, quests and
dialogue, cinematics, interfaces, materials, sounds, effects. Modelling, sculpting, rigging and
animation made by hand stay in Blender, linked live: a change in Blender shows in the editor and in
the running game within a second, through an add-on of Blender's and an optional crate of the
editor's. glTF stays the native format; FBX and OpenUSD are optional importers.

**Gameplay without code.** Graphs for each domain, which the stages already plan: dialogue and
quests, the states of animation, materials, sounds, characters' AI, and Stage 9b's track of combat.
The logic of a game in general is code, Rust or scripts, which agents write. A visual scripting of
every logic stays in the horizon.

**Versions.** The editor knows Git, with LFS for large files: the state of files, their history,
and levels merged by [ADR-0208](0208-the-engines-promises-and-systems.md)'s tool. Lore and
Perforce come later, as optional crates.

**Several people and agents in one level, at Stage 11.** Several editors and agents change one level
at once, each seeing the others' changes as they are made, on the editor's commands and journal.
It moves from the horizon's tools of large teams.

**Play, rewound and on a device.** At Stage 7, while a game is played in a development build, its
last seconds are kept: a person or an agent goes back, steps frame by frame, reads each value at
each instant, then plays on, through the deterministic simulation. At Stage 9b, with packages, the
editor builds a game for a paired device, sends it, starts it and keeps driving it, in one action.

**The editor's lowest machine.** The modest tier, comfortable: its 3D views lighten, its interface
never.

**Games' interfaces on uwui's core, with a layer of the engine's.** As the largest engine builds
its games' interfaces over its editor's framework: uwui's core, its tree, layout, text, focus,
gamepads, accessibility, styles and signals, and, in the engine, a layer for what games alone need:
drawing in the render graph with the game's materials, interfaces within the world, 3D within an
interface, the game's time, its pause and its slowing. M6d.10 confirms it by measuring a HUD of
*Hunna Wood*'s and a menu of a strong identity: within the interface's budget, no allocation at a
frame, uwui's core built without the system's services. If the measure fails and uwui cannot take
the fix as a general need, games' interfaces get a framework of their own. At Stage 11, a visual
editor of games' interfaces, the engine's own, works over uwui's format of layouts, which agents
write too. What this asks of uwui is in [`uwui-needs.md`](../uwui-needs.md).

## Alternatives considered

- **Consoles left to other companies**, as one open engine does: nothing to do, but quality and
  dates out of the owner's hands, and an income lost.
- **The Steam Deck as Linux alone**: nothing more, but sleep, the keyboard on screen and the sizes
  of text forgotten.
- **Virtual reality as a stage of its own**: it would come, but none of the profiles asks for it.
  **Never**: nothing to keep open, but a market shut for the engine's users. **No mobile, ever**, as
  ADR-0098 had it for the owner's games: no constraint, but the largest market of games shut forever.
- **The channel on the user's machine alone**: no surface on the network, but the Steam Deck
  measured only by working on it.
- **An assistant within the editor alone**, or **the panel alone**: the owner chose both, the
  assistant optional.
- **Everything made in the editor**, modelling and animation included, as the largest engine does:
  one place, but an enormous work to do worse than Blender, free, does.
- **A visual scripting of every logic at Stage 11**: designers alone without an agent, but a large
  system to hold to the bar, and graphs that nobody can read.
- **A versioning of the engine's own**: made to measure, but enormous, where Lore now exists, open,
  in Rust. **No versioning in the editor**: nothing to do, and no merge of levels within it.
- **Several people in one level at the end of Stage 6c**: agents served sooner, Stage 7 later.
  **In the horizon**: agents each on their copy, merged afterwards.
- **Rewinding at Stage 11**: later, with the complete editor; the profiler and the determinism of
  Stage 7 make it possible sooner.
- **The editor on the Steam Deck too**: very wide, but an editor designed for 1280 × 800 and a
  gamepad. **The high tier alone**: nothing to lighten, and many authors shut out.
- **uwui whole for games' interfaces**, as ADR-0190 wrote: the simplest, but no place for what games
  alone need. **A framework of games' own**, on the lowest crates of text, layout and accessibility:
  full control, but focus, input of text, accessibility, styles, reloading and the inspector written
  twice, and two systems to learn.
- **No visual editor of interfaces**: less to build, artists who do not code depending on an agent.

## Consequences

- ADR-0099's server gains rights, previews, tools by families and replays, and opens to a paired
  machine in development builds; the threat model grows with each.
- The editor gains, at Stage 7, rewinding; at Stage 9b, the track of combat and the launch on a
  device; at Stage 10a, the tools of placement; at Stage 11, several people in one level, the panel
  of agents and the optional assistant, the visual editor of games' interfaces, Git within it.
- Stage 6d's M6d.10 weighs games' interfaces as above; its result may reopen this decision.
- Steam's crate is needed before the first game is sold, around Stage 9b.
- A console's layer lives outside this repository; the engine's CI never sees its code.
