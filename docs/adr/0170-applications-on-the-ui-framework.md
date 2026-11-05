# 0170. Applications on the engine's UI framework

- Status: Accepted; amends [0132](0132-the-engines-ui-framework.md) (a third client, applications,
  beside the editor and games), [0098](0098-the-games-the-engine-aims-at.md) (a stage between the
  editor and the runtime systems, which takes from Stage 7 what the framework gains for games;
  Stage 9c's work ships with 0.13) and [0149](0149-the-looks-foundations.md) (Stage 6d, not
  Stage 7, grows the transitions that Stage 6b declares); amended by
  [0171](0171-effects-of-the-interface.md) (Stage 6d lays the foundations of the interface's
  effects, and measures the power that it draws),
  [0173](0173-the-platforms-of-applications.md) (the platforms that Stage 6d aims at),
  [0178](0178-two-pilots-and-the-frameworks-name.md) (both programs ported during Stage 6d; the
  framework named now, in a repository of its own once its drawing leaves the renderer)
  and [0181](0181-uwuis-scope.md) (uwui's scope: the base of the owner's programs, in a
  repository of its own)
- Date: 2026-11-04

## Context

[ADR-0132](0132-the-engines-ui-framework.md) made the engine's UI framework, `slime-ui`, drawn
through the renderer's canvas by `slime-ui-render`, for two clients: the editor and the interface
of games. It runs headless, keeps a retained tree of its own, lays out with taffy, shapes text with
parley, and tells agents and screen readers what its elements mean.

The owner also writes desktop programs outside this repository, in Rust: among them Slatty, a
launcher of games written with Iced, and a music player written with egui, whose interfaces were
both being rewritten with GPUI, the framework of the Zed editor. On 2026-11-04 the owner set GPUI
aside and asked whether these programs should move onto the engine's framework instead, for the
long term, time not being a constraint. What was weighed:

- They run where the engine runs, Linux, Wayland first, and Windows. GPUI follows Zed's needs,
  macOS first; the trial needed a fork of it, to keep Wayland's fractional scale and to dither its
  gradients.
- The engine's framework is built to be read and driven by agents through the engine's MCP server,
  and these programs are developed by agents, as the engine is.
- One framework for the engine and these programs: what is fixed for one serves the others, and
  the programs use the framework every day, as the editor does.
- Against it: the framework lacks much that the programs need today, their needs could pull it
  towards the desktop and away from games, its API changes freely before 1.0, and the engine does
  not run on macOS yet.

The session of each program wrote an inventory of what its interface asks of a framework, with the
code that asks it, and each need was checked against the engine's code. Of 45 needs, 18 exist, 7
exist in part and 20 are missing:

- The window and the program around it: the window's identity, its application id and its minimum
  size; going fullscreen and back; bringing the window to the front; drawing an interface without
  paying for the 3D renderer, which `Renderer::new` builds whole; drawing without a GPU, which the
  launcher's Iced does on the CPU; being driven by agents, which only games and the editor are.
- Input: masked fields, double clicks, the pointer's shapes beyond text and resizing, smooth and
  sideways scrolling, files dropped on the window, gamepads.
- Drawing: several shadows on an element, gradients of several stops and radial ones, dithered;
  transforms; images read from files and the network, in a cache that frees the GPU's memory;
  cropping an image to cover its box; textures updated at each frame, for animated covers; lines of
  several segments, meshes or vector paths.
- Motion: transforms animated, curves and springs, delays in cascade, animations driven by
  scrolling, a frame rate capped for each animation.
- Lists and text: virtual lists that scroll sideways, a table's header that stays as its rows
  scroll, text styled by ranges, the music player's lyrics colored word by word.

Most of these serve games too: a line of dialogue whose names are styled within it, video in a
menu, transforms and gradients. Several were already listed for Stage 7's interface of games.

What the programs measured with their present frameworks, on the owner's machine and its 4K
display at 240 Hz, and which the engine's framework must reach:

- The launcher, with Iced: its library of 10 000 games shown in 86 to 91 ms; scrolling at 4.15 ms a
  frame by median, no frame over 16.7 ms; no CPU time at rest; 116 to 135 MB of memory.
- The music player, with egui: nothing drawn at rest, the display's rate held while it scrolls and
  animates, about 300 MB as it starts.

And what the GPUI trial taught: a framework that draws again at each refresh while at rest costs a
tenth of a core, which shows under Xwayland when only Wayland is measured; images need a bounded
cache that frees their place in the GPU's memory, without which the launcher's memory stayed at
455 MB after scrolling, against 116; and the cost of a frame lies in the number of elements built,
more than in how they are built.

## Decision

The owner's decisions of 2026-11-04.

**Applications are the framework's third client.** Beside the editor and games, the framework
serves *applications*: desktop programs built on it that are neither a game nor the editor. It
stays one framework, with one way to build an interface: an application builds its tree, styles it
and reads its actions as the editor does.

**Games and the editor first.** An application's need enters the engine only if it is general,
serving a game or the editor too, or costing them nothing; what serves one application alone stays
in that application, as do its services and what it does with the system around it, media keys, a
store's sign-in or a player in another process.

**A boundary.** The framework, and what runs an application, depend on neither the world, a game's
host, the editor nor games, so that the framework can leave this repository later without its code
changing.

**A shell that runs an application.** A crate of the engine runs an application: its window, the
GPU and the drawing of its interface, its frames drawn only when something changes, woken from any
thread, and the development channel, through which agents drive it as they drive a game; no world
and no simulation. An application that draws no scene does not pay for the 3D renderer, whose cost
is measured first. The crate's name and design are decided at the stage's start.

**Stage 6d, between the editor and the runtime systems.** A stage of its own follows Stage 6c and
comes before Stage 7, released as 0.8, each later release moved by one. It takes from Stage 7 what
the framework was to gain there and that does not depend on a game's runtime: animation beyond the
transitions of Stage 6b ([ADR-0149](0149-the-looks-foundations.md)), transforms, masks, text styled
by ranges, the effects on text, and focus driven by gamepads, designed once for games, the editor
and applications. Stage 7 keeps the interface on surfaces of the world, the screen readers of
games, and UI documents. The stage has six parts: the shell, images, drawing, motion, text, input
and widgets, and the pilot. Whether the framework draws without a GPU, and whether shapes beyond
boxes are vector paths or meshes, are decided at its start.

**Both programs' needs.** The stage covers what both programs need to match their present
frameworks, not the launcher's needs alone. The launcher is the pilot: it is ported onto the
framework in its own repository during the stage, with a checkpoint where the owner uses it. The
music player is ported after the stage, in its own repository.

**Exit.** The launcher runs on the framework at least as well as on Iced, on each of the figures
above; every need that the music player's inventory marks as indispensable exists, shown by an
example and its tests; nothing is drawn at rest, under Wayland and under Xwayland.

**Done well before done soon.** The owner wants both programs usable as soon as they can be, and
the stage to take a year rather than leave debt that will be regretted. It is planned in milestones
at its start, with the owner, as every stage is.

**In this repository, under the engine's name.** The framework stays in this repository: a change
of the framework, of the editor and of the game is one commit and one run of CI, and the editor and
the game test the framework at each push. It moves to a repository of its own, and may take a name
of its own, the day it is published for other people; the boundary above keeps that move
mechanical. Until then it is the engine's UI framework, Slime UI in prose, and its crates keep
their names.

## Alternatives considered

- **The programs keep other frameworks**, GPUI, egui or Iced: no work for the engine, and two ways
  to build an interface in the owner's work for good. GPUI owns its window and its GPU device,
  follows Zed and needed a fork; egui lays everything out again at each frame and keeps no
  identity that agents could name ([ADR-0132](0132-the-engines-ui-framework.md)); Iced brings
  `ttf-parser`, which the engine's checks refuse, and tells screen readers nothing.
- **The stages kept in their order**, the shell and the pilot after Stage 6c, animation and
  transforms at Stage 7: nothing moves for the game, but the launcher waits for Stage 7 for its
  motion, and animation is designed for games first, then reshaped for applications.
- **A track beside the stages**: nothing waits, but two pieces of work on the same crates at once,
  which [AGENTS.md](../../AGENTS.md#working-with-several-agents) keeps to one agent at a time, and
  which drift apart.
- **The launcher's needs alone**: the launcher sooner, but drawing and motion taken up a second
  time for the music player.
- **A repository of its own now**: the framework stands on twelve of the engine's twenty crates,
  reflection, serialization, color, input, the platform, the GPU layer, the renderer with its assets
  and its tasks, the bridge, the development channel and the MCP server, which would leave with it;
  each change of the framework would touch two repositories, and drawing through the renderer's
  canvas, which ADR-0132 decided, would be decided again.
- **A name of its own now**: code, ADRs and documents renamed for no technical gain, before anyone
  outside uses the framework.

## Consequences

- The roadmap gains Stage 6d, and Stage 7's interface of games keeps what did not move; the
  releases after 0.7 move by one, Stage 9c's work shipping with 0.13 and Stage 11 releasing 0.15
  before 1.0.
- `slime-ui` gains a client that uses it every day, and requirements stated as figures measured
  with other frameworks.
- Files dropped on the window under Wayland still wait for winit 0.31, for applications as for the
  editor ([ADR-0155](0155-the-editors-design.md)).
- The applications follow the engine's releases: the framework's API still changes freely before
  1.0.
