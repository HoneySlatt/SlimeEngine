# What SlimeEngine asks of uwui

uwui is the owner's interface framework, which leaves this repository for
[its own](https://github.com/HoneySlatt/uwui) at Stage 6d
([ADR-0194](adr/0194-stage-6d-uwui-leaving-the-engine.md)). The engine depends on it, never the
reverse: its editor, the interfaces of its games and its SDK draw with uwui. This file lists what
they ask of it. It separates the general needs, which uwui may hold, from the engine's own, which
the engine builds on them.

It comes from two sources:

- what the editor and the game draw with uwui today, in this repository, and the plan of the
  editor's last milestones ([`stage-6c.md`](stage-6c.md));
- the owner's interview of 2026-11-07 on what the engine is to become
  ([ADR-0207](adr/0207-what-slimeengine-is-to-become.md),
  [ADR-0208](adr/0208-the-engines-promises-and-systems.md),
  [ADR-0209](adr/0209-platforms-the-editor-and-agents.md)), before any code;
- the owner's decisions of the same day that measure the engine and uwui against the market
  ([ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md) to
  [ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md)).

It serves the question that [M6d.10](stage-6d.md#m6d10--uwui-designed-as-a-project) weighs: whether
games' interfaces take uwui whole, its core with a layer of their own, or a framework of their own.
ADR-0209 chose the core with a layer of the engine's, to be confirmed by a measure; [its section
below](#for-m6d10) says what the measure needs. Nothing here is measured yet.

## How to read it

- uwui takes a need only if it is **general**: several kinds of applications need it. A need that
  serves only the engine, or only one benchmark, stays in the engine, in the layer of games'
  interfaces or in the editor ([ADR-0190](adr/0190-uwuis-requirements.md)).
- **Mark**: *indispensable* when the editor, a game or a decision of ADR-0207 to ADR-0220 cannot be
  met without it; *wished* otherwise. Each mark names the first stage that needs it: uwui's 1.0,
  when one of uwui's own promises that the engine relies on needs it; Stage 6c's resumption, when
  the editor moves onto uwui's 1.0; Stage 7, when games draw their interfaces; Stage 9b, the
  menus' timeline; Stage 11, the complete editor.
- **In uwui's requirements**: whether [ADR-0190](adr/0190-uwuis-requirements.md),
  [ADR-0205](adr/0205-signals-change-uwuis-interface.md) and
  [uwui's roadmap](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md) hold the need, as
  read on 2026-11-07 and updated as later ADRs decide. *Yes*: they hold it. *Precision*: they hold it in general terms, and this list
  says exactly what is needed. *New*: they do not hold it.

## How the engine's interfaces change

The editor and games change their interfaces in two ways that applications rarely mix.

- **At every frame, in a game.** A HUD shows values that move at each frame: health, stamina, a
  timer, a minimap, damage that rises and fades. The game draws at its tier's rate, up to 240
  frames a second, and its interface is drawn into each frame. These values are live values
  ([ADR-0205](adr/0205-signals-change-uwuis-interface.md)), written by the simulation's thread and
  read as the element is drawn, without running the interface's code.
- **On demand, in the editor.** The editor draws nothing at rest
  ([ADR-0155](adr/0155-the-editors-design.md)); a change of the world, a drag, an agent's request
  asks for a frame. A level of 100,000 entities changes by key: an entity renamed, moved in the
  hierarchy, a component added.
- **In both:** menus, inventories and panels open as layers over what stays; the game pauses and
  slows while its menus run at real speed.

## General needs

### The host and the frame

| Need | The engine's use | Mark | In uwui's requirements |
|---|---|---|---|
| uwui embedded in a host that owns the loop, the engine's `slime-app`, which calls it at each frame it runs | The editor and every game | Indispensable: Stage 6c | Yes: embedded in a host |
| Drawing into a target that the host gives, composited over or under the host's image, in the host's pass, at the host's rate: every frame in a game, on demand in the editor | The HUD over the 3D image; the editor's panels around its viewports | Indispensable: Stage 7 | Precision: the canvas on wgpu records into the host's encoder, which the host submits (M6d.4b1b); drawing at every frame into a pass of the host's is to say |
| A root that is not a window: its size, scale and safe area given by the host, several roots in one frame | A game's interface at its resolution, scaled for a television across the room; interfaces placed in the world | Indispensable: Stage 7 | Yes: roots that are not a window ([ADR-0132](adr/0132-the-engines-ui-framework.md)) |
| A clock given by the host, for each root: motion and timers that follow the game's time, paused and slowed, or real time | Menus that run while the game is paused; a HUD that slows with it | Indispensable: Stage 7 | Yes: each root given its time and speed at each frame, the game's interpolated between its steps; the application's motion and timers follow it, hover, press, focus and uwui's own timings stay on the host's clock; what runs during a pause, a root of its own ([ADR-0227](adr/0227-the-time-that-motion-reads.md)); met in uwui's core at M6d.5b (`Ui::set_root_clock`), its tasks' timers with its executor ([ADR-0251](adr/0251-the-engines-host-clock-and-its-times.md)) |
| What lies behind an element read from the host's image, for its backdrop: blur, tint, glass | A menu over the blurred game; the editor's floating panels | Wished: Stage 7 | Precision: backdrops over what uwui drew; over the host's image is to say |
| An element whose drawing the application gives, its shader or a scene of the engine's, with the pointer's raw actions held during a drag | The editor's viewports and their handles; a character turning in an inventory | Indispensable: Stage 6c | Yes: a view drawn by the application's shader; a scene of SlimeEngine in an element |

### Input and focus

| Need | The engine's use | Mark | In uwui's requirements |
|---|---|---|---|
| Input given by the host, which decides what reaches the interface and what the game keeps, and learns what the interface consumed | A click on a menu that does not fire the weapon | Indispensable: Stage 7 | Precision |
| One model of focus for the keyboard, gamepads and accessibility; menus driven by a gamepad alone | Every game's menus; the couch density | Indispensable: Stage 7 | Yes |
| The keyboard on screen driven by a gamepad | A name typed on the Steam Deck | Indispensable: Stage 7 | Yes |
| Shortcuts remapped by the user, with chords, from files of keys; several sets laid over each other | The editor's `.keys.kdl` files, Blender's way among them ([ADR-0160](adr/0160-two-viewports-and-blenders-way.md)) | Indispensable: Stage 6c | Yes |
| A primary modifier, Ctrl or Cmd, in the format of shortcuts, a menu's shortcut shown per platform | The editor's shortcuts on macOS, the files of keys migrated once ([ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md)) | Indispensable: Stage 6c | Yes: M6d.3 |
| Actions declared once, which feed the menus, the system's native menus among them, the shortcuts, the palette of commands and agents | The editor's operations, each one declared once ([ADR-0218](adr/0218-one-core-for-agents-in-uwui.md)); a game's input actions | Indispensable: Stage 6c | Precision: actions declared once for agents, at M6d.9b; that menus, shortcuts and the palette read them is to say |
| Input methods and every writing system in fields | A game's chat and names; the editor's fields | Indispensable: Stage 7 | Yes |

### Drawing and motion

| Need | The engine's use | Mark | In uwui's requirements |
|---|---|---|---|
| No allocation at a frame once an interface is steady, counted in CI | A HUD at 240 frames a second on the minimal tier | Indispensable: Stage 7 | Precision: counters in CI exist for the engine ([ADR-0186](adr/0186-counters-and-ratios-in-ci.md)); allocations per frame as uwui's budget is to say |
| The interface's GPU time within 0.5 ms on each tier's machine, the minimal one included | Every game | Indispensable: Stage 7 | Precision: the engine's budget ([`ROADMAP.md`](ROADMAP.md#budgets)); uwui's own budgets do not name game tiers |
| Transforms, masks, groups drawn apart, motion with curves and springs, interrupted without a jump | Menus of a strong identity, as the toon role-playing game's | Indispensable: Stage 7 | Yes: the first stage |
| A timeline that animates an interface without code, written by designers and by agents | The menu of reference of games, its timeline at Stage 9b with the sequencer ([ADR-0215](adr/0215-the-engine-measured-against-the-market.md)); the engine's applications share it | Indispensable: Stage 9b | New |
| Text styled by ranges, variable fonts whose axes move, colour emoji, right to left | Dialogue with names and emphasis; damage numbers | Indispensable: Stage 7 | Yes |
| Lists and trees of 100,000 rows changed by key, the focus, selection and scroll kept | The editor's hierarchy and asset browser | Indispensable: Stage 6c | Yes: a list fed by its changes by key |
| Docking, floating panels as windows of the system, several windows | The editor ([ADR-0174](adr/0174-layouts-kept-floating-panels-and-closed-panels.md)); panels in windows of their own at Stage 11 | Indispensable: Stage 6c | Yes |

### Text, themes and accessibility

| Need | The engine's use | Mark | In uwui's requirements |
|---|---|---|---|
| Themes and densities from files, the editor's dense look, a game's theme its own | The editor's look, in the official theme Pro ([ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md)); each game's identity | Indispensable: Stage 6c | Yes |
| Themes that change a component's structure, not only its values, and no colour hidden in a component's defaults | A game's menus that look like no other game's ([ADR-0215](adr/0215-the-engine-measured-against-the-market.md)); the editor's Pro | Indispensable: Stage 6c | Yes: a theme's parts as data, decided ([ADR-0222](adr/0222-what-a-theme-is.md)), built in uwui's first stage, with the part of a style that a theme adds ([ADR-0229](adr/0229-styles-typed-as-declarations.md)); no colour hidden, M6d.2b1 ([ADR-0230](adr/0230-the-cores-roles-and-each-kinds-defaults.md)) |
| Translation through Fluent, the language changed live, and a fake language of test | The editor's texts through tables from Stage 6c's resumption ([ADR-0207](adr/0207-what-slimeengine-is-to-become.md)); every game | Indispensable: Stage 6c | Yes, but the fake language: Precision |
| The tree of what an interface means, for screen readers, from a root that is not a window | Games' menus read aloud ([ADR-0208](adr/0208-the-engines-promises-and-systems.md)) | Indispensable: Stage 7 | Precision: AccessKit from a window exists; from another root is to say |
| Sizes of text that the user or the game chooses, honoured by every component | The guidelines that games follow | Indispensable: Stage 7 | Yes: the system's size honoured |
| The system's preferences, light or dark, contrast and forced colours, less motion and transparency, the size of text and saving power, read by a crate of uwui's that the engine's platform layer calls too | The editor following them on Linux, where winit reads none; games' menus under forced colours and less motion | Indispensable: Stage 6c | Yes: a reader of the system's preferences, shared by uwui's hosts and the engine ([ADR-0226](adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)) |

### Documents, agents and tools

| Need | The engine's use | Mark | In uwui's requirements |
|---|---|---|---|
| Layouts written back by a tool without losing the author's order and comments | Agents that write layouts, and the agent bench's screens from mock-ups ([ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md)); the visual editor of games' interfaces later ([ADR-0209](adr/0209-platforms-the-editor-and-agents.md)) | Indispensable: uwui's 1.0 | Yes: KDL 2.0, decided; read, reloaded and written back with its order, formatting and comments kept by uwui's own reader, built in uwui's first stage ([ADR-0225](adr/0225-layouts-written-in-kdl-reached-from-code-by-their-names.md)) |
| Layouts from outside read within limits of size, memory and time, never crashing | Mods that bring interfaces ([ADR-0208](adr/0208-the-engines-promises-and-systems.md)) | Indispensable: Stage 8 | Yes: content from outside within limits; the layouts' reader bounded in depth and fuzzed ([ADR-0225](adr/0225-layouts-written-in-kdl-reached-from-code-by-their-names.md)) |
| One core for agents: a protocol that no transport binds, the agent named with the person's consent, numbered requests and events, attaching to a program already running, rights, previews before applying, tools found by families, and families that the program adds | The engine's channel and MCP server built on it, with its families of the world, play and editing ([ADR-0218](adr/0218-one-core-for-agents-in-uwui.md)) | Indispensable: Stage 6c | Yes: M6d.9a, cut in five ([ADR-0246](adr/0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md)) |
| Commands, undo and their journal: whether uwui or the engine owns them | The editor's steps, each with its author and time; an agent's action undone as a person's | Indispensable: Stage 6c | New: decided at M6d.10 ([ADR-0218](adr/0218-one-core-for-agents-in-uwui.md)) |
| The live inspector, absent from builds that ship | The editor and games while developed | Wished: Stage 7 | Yes |
| Tests without a window, by paths, images of reference and counters | The engine's CI | Indispensable: Stage 6c | Yes |

### Platforms and stability

| Need | The engine's use | Mark | In uwui's requirements |
|---|---|---|---|
| The core built and run with fonts, files, the clipboard and threads all given by the host, none taken from the system; checked by a build that uses none | Consoles one day, through private layers ([ADR-0209](adr/0209-platforms-the-editor-and-agents.md)) | Wished: before consoles | New |
| Linux and Windows first, the Steam Deck as a platform of the first rank | Every game | Indispensable: Stage 7 | Yes |
| Services of files, settings, the network and long work, the same on every platform, the web and the phones included | The editor's and the tools' files and settings, and their long work, written once for every platform, as uwui's budget of porting asks ([ADR-0213](adr/0213-uwuis-reach-its-lead-its-themes-and-agents.md)) | Indispensable: uwui's 1.0 | Yes: weighed at M6d.10 |
| Semantic versioning kept strictly after uwui's 1.0 | The engine's façade, stable after its own 1.0 ([ADR-0207](adr/0207-what-slimeengine-is-to-become.md)) | Indispensable: the engine's 1.0 | Yes |

### What the owner's shell and compositor ask

Not the engine's: since [ADR-0266](adr/0266-uwui-targets-the-owners-shell-its-lock-and-login-screens-and-what-yuna-draws.md),
uwui draws the owner's shell, Hollow, its lock and login screens, and the interfaces that the
compositor Yuna draws itself. Kept here, beside the engine's, as uwui's general needs of hosts;
Yuna's side is its ADR-0063.

| Need | Its use | Mark | In uwui's requirements |
|---|---|---|---|
| A surface of Wayland's layer shell as a target: its layer, its anchors, its exclusive zone, its keyboard's interactivity | Hollow's bar and panels | Indispensable: Hollow | New |
| A surface of the session lock as a target, one for each output, its input alone while locked | Hollow's lock screen | Indispensable: Hollow | New |
| A host that lends a Vulkan device of its own, its queue and its targets, rather than a wgpu device | Yuna's own interfaces drawn on its Vulkan device | Indispensable: Yuna | New: `uwui-render` takes a wgpu device today |
| A failure of uwui inside a host caught and recovered from: the interface dropped and built again, the host going on | Yuna, which a fault of its interfaces must never take down | Indispensable: Yuna | New |
| uwui's time in each frame of a host counted and checked in CI | Yuna's frame budget | Indispensable: Yuna | Precision: the counters exist ([ADR-0186](adr/0186-counters-and-ratios-in-ci.md)); a host's frame budget is to say |
| Nothing drawn, nor asked of the GPU, while the host shows a game in full screen and no interface is open | Yuna over a game | Indispensable: Yuna | Precision: rest at zero is uwui's; a host that holds it whole is to say |

## SlimeEngine's own

These stay in the engine. They ask uwui only for the general needs above.

- **The layer of games' interfaces** ([ADR-0209](adr/0209-platforms-the-editor-and-agents.md)):
  drawing into the render graph with a game's materials and passes; effects on an element that a
  game's shader makes; 3D within an interface; the game's clock fed to uwui's roots
  (`Ui::set_root_clock`): the game time that the frame shows, interpolated between its last two
  steps as [ADR-0227](adr/0227-the-time-that-motion-reads.md#what-a-roots-clock-drives) gives
  it, and its speed, 0 while paused; the layers that open over a root follow its clock.
  What runs while the game is paused, a pause menu or a dialog over the paused game, is a root
  of its own, on the host's clock, sharing its game's window: in the game's root, a switch's
  travel, a panel's entrance or a task's timeout stop with the game
  ([ADR-0227](adr/0227-the-time-that-motion-reads.md)).
- **Interfaces in the world**: on surfaces, facing the camera or not, lit or not, and one day in
  virtual reality; names over hundreds of players and damage that rises, drawn by instances.
- **Input actions** of Stage 7, mapped onto uwui's focus and actions; the glyphs of each controller
  from Steam Input.
- **Menus spoken aloud** when no screen reader runs, as the Xbox Accessibility Guidelines ask, by
  reading uwui's tree of what an interface means; the game's sound lowered while it speaks.
- **The editor's panels**: viewports, handles, the hierarchy, the inspector driven by reflection, the
  asset browser, the history; at Stage 11 the visual editor of games' interfaces, the panel of
  agents and the optional assistant, hosted and paid
  ([ADR-0214](adr/0214-the-engine-under-a-licence-of-its-own-with-a-royalty.md)).
- **The dev overlay** of games ([ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)).
- **The engine's families of tools for agents**, the world, play and editing, added on uwui's core
  for agents; the channel, rights and previews are the core's
  ([ADR-0218](adr/0218-one-core-for-agents-in-uwui.md)).
- **What an interface keeps in a game's save**, and its state rewound with the game
  ([ADR-0209](adr/0209-platforms-the-editor-and-agents.md)).

## For M6d.10

ADR-0209 confirms games' interfaces on uwui's core with a layer of the engine's, once a measure
shows it holds. The engine proposes what the measure is, for M6d.10 to decide with the owner:

- **What is drawn**: a HUD of `games/open-world`'s
  ([ADR-0219](adr/0219-test-games-in-place-of-hunna-wood.md)) whose values change at each frame,
  health, stamina, a compass and damage rising; a menu of a strong identity, animated, with an
  inventory of 500 items and a character shown in 3D; a dialogue with text styled by ranges; all
  driven by a gamepad alone.
- **Where**: the owner's machine at 3840 × 2160 at 240 frames a second, and the Steam Deck at 1280 ×
  800.
- **Criteria**: the interface's GPU time within its budget of each tier; its CPU time at each frame
  within a share of the main thread that M6d.10 sets; no allocation at a frame once steady; nothing
  drawn while the game is paused and hidden; uwui's core built without the system's services.
- **If it fails**, and uwui cannot take the fix as a general need, games' interfaces get a framework
  of their own, built on the lowest crates of text, layout and accessibility (ADR-0209).

## What Stage 6c's resumption needs first

The editor moves onto uwui's 1.0 when Stage 6c resumes, after Stage 6e
([ADR-0190](adr/0190-uwuis-requirements.md),
[ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md)). It needs, before anything else: the
host and its frames on demand; an element drawn by the application with the pointer held; lists
and trees of 100,000 rows; docking and floating panels; files of keys with sets laid over each
other and the primary modifier; the editor's look in the theme Pro; translation tables for every
text; and uwui's core for agents, with the editor's operations declared on it.
