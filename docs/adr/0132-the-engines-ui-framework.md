# 0132. The engine's UI framework

- Status: Accepted; amends [0098](0098-the-games-the-engine-aims-at.md) (the framework's design
  and the crates it builds on, decided; the dev overlay rewritten on the framework, and egui
  removed, at Stage 6b), [0052](0052-amend-roadmap-before-0-1.md),
  [0031](0031-dev-overlay-with-egui.md), [0081](0081-stage-5a-the-world.md) and
  [0105](0105-egui-fonts-ship-with-the-dev-overlay.md) (the dev overlay and its world panel rewritten
  on the framework, and egui and its fonts removed, at Stage 6b) and
  [0115](0115-what-the-gpu-layer-gives-the-editor.md) (a texture of one channel, and a scissor apart
  from the viewport); amended by [0137](0137-the-canvas.md) (no scissor apart from the viewport; the
  interface blends in sRGB's encoding), [0140](0140-the-tree-of-elements.md) (the marks of an
  element's style and meaning come with the milestones that use them),
  [0143](0143-what-agents-see-of-the-interface.md) (an element's value comes with the first element
  that holds one) and [0145](0145-text-edited.md) (the engine composes the text of input methods
  and keeps a field's text, parley giving the selection's moves)
- Date: 2026-10-22

## Context

[ADR-0098](0098-the-games-the-engine-aims-at.md) decided one UI framework of the engine's own,
GPU-rendered, for the editor and for the UI of games, egui staying only for the dev overlay until
something does at least as well ([ADR-0052](0052-amend-roadmap-before-0-1.md)). It left the
framework's design, retained, immediate or hybrid, and the crates it builds on, to Stage 6, each
dependency asked for then. [ADR-0131](0131-stage-6b-and-stage-6c.md) builds the framework in Stage
6b, before the editor's panels of Stage 6c.

The owner chose on 2026-10-22 how the design was studied. Four agents, each alone, read the
repository and proposed a design from another starting point: what other engines learned, the
Rust ecosystem as it stands, this engine's own architecture, and the needs of the games of
ADR-0098. A fifth measured the candidate crates in prototypes outside the repository, on the
owner's machine (an AMD Ryzen 7 5800X, Rust 1.99.0), in release builds; the agents' reports and the
prototypes are kept outside the repository, as the audit's report was. What they found:

- **Every engine that serves its editor and its games with one UI ended on a retained tree.**
  Unity's UI Toolkit is a tree of elements styled by style sheets, apart from its game objects;
  Unreal's Slate is a tree of widgets of its own, under UMG's assets; Godot's editor and games use
  its Control nodes, nodes of its scene tree as the rest of a game is; Fyrox's editor and games use
  fyrox-ui. The way there cost years: Unity's editor ran on IMGUI, an immediate-mode UI, before UI
  Toolkit; Slate's bindings, read again at every frame, needed invalidation added over several
  versions; Bevy, whose UI elements are entities of its world, still builds the reactivity and the
  templates its editor waits for; O3DE's two stacks, Qt for the editor and LyShine for games, left
  the games' to wither.
- **No Rust framework fits the engine.** iced, Xilem and Masonry, Floem, Makepad, Bevy UI and GPUI
  create their own GPU device or own the window and its event loop, where `slime-gfx` owns the
  device and `slime-platform` the window ([ADR-0011](0011-windowing-with-winit.md),
  [ADR-0016](0016-gpu-layer-with-wgpu.md)), and Stage 9c may put native backends under
  `slime-gfx`. Vizia and Freya draw through Skia, a C++ library. Slint's licenses are GPL-3.0 or
  terms that are not OSI-approved, which [ADR-0006](0006-dependency-policy.md) refuses. iced,
  fyrox-ui, yakui and every user of cosmic-text bring in `ttf-parser`, unmaintained
  (RUSTSEC-2026-0192), which the dependency checks refuse. None offers what the games ask: text
  effects, UI in three dimensions, focus for gamepads, UI authored as data.
- **The Rust ecosystem has converged on three building blocks**: parley for text, over HarfRust,
  Skrifa and ICU4X; taffy for layout; AccessKit for accessibility. Bevy, Blitz, Masonry, Slint and
  egui use them, or are moving to them. HarfRust 0.12 and Skrifa 0.44, which parley 0.12 uses, are
  the versions that egui 0.36.2 already builds in the engine.
- **The measurements, on the owner's machine:**
  - parley 0.12 lays out 1 000 labels of Latin text in 7.0 ms, and a paragraph of 2 000 characters
    at 400 pixels in 0.6 ms in English or Chinese, 1.0 ms in Thai with the `complex-scripts`
    feature, 1.3 ms in Hindi and 5.4 ms in Arabic with Latin words mixed in. Bidirectional order is
    right, no glyph is missing, and an emoji sequence is one color glyph. A change of width keeps
    the shaping (20 to 45 µs to break the lines again); an edit lays the paragraph out again (0.67
    ms in English, 6.1 ms in Arabic). Thai breaks between its words only with parley's
    `complex-scripts` feature, which adds 3.1 MB to a program. Japanese takes 172 ms when its font
    is found by fallback with the locale `ja`, since fontique does not keep that lookup, and 1.3 ms
    when the font is named.
  - cosmic-text 0.19, the alternative, is faster on Arabic (2.1 ms), slower on Chinese (4.6 ms) and
    emoji, breaks Thai inside its words, and brings `ttf-parser`.
  - swash 0.2 rasterizes a Latin glyph in 1.3 to 4.9 µs from 14 to 64 pixels, a Chinese one in 5.5
    to 12.5 µs, a color emoji in 0.25 to 0.45 ms.
  - taffy 0.15 lays out a tree of 12 001 elements in 9.8 ms, then again in 0.88 ms after one
    element changes; with nothing changed, its rounding still walks the tree, 0.56 ms.
  - A tree of 100 000 rows laid out only where it shows costs 0.45 ms per frame scrolled by a page,
    60 labels shaped, and 12 µs standing still. The dev overlay's world panel, which lays out every
    row, cost 125 to 145 ms per frame at 100 000 entities when it was measured, before M6a.3 (G46).
  - AccessKit builds the tree of 1 000 elements in 0.22 ms, and the update of 10 in about 1 µs.
  - Built alone, from clean, on 16 threads: parley in 8.7 s (debug) and 11.9 s (release), taffy in
    2.6 and 3.9 s. AccessKit's adapter brings, by name, 46 crates that the engine does not build
    today on Linux, through D-Bus, and 3 on Windows.
- **What the engine lacks.** `slime-platform` reads keys, the mouse and the scale factor of one
  window; it has no input methods, clipboard, cursor shapes, dropped files, dialogs or raw motion of
  the mouse. winit 0.30.13 has input methods on every system, and files dropped on Windows and X11,
  not on Wayland: its 0.31, in beta, adds them. `slime-render` draws views, and lines and unlit
  instances into them ([ADR-0129](0129-drawing-for-the-editor.md)); nothing draws rounded
  rectangles, text or images in two dimensions but egui, through the `wgpu` feature that
  [ADR-0031](0031-dev-overlay-with-egui.md) allows the overlay.

The owner chose the study's method, then took the decisions below on 2026-10-22, one at a time, each
among the options that "Alternatives considered" lists: the base of the design, where the interface
is drawn, when screen readers come, when egui leaves, and the dependencies. The fonts are in
[ADR-0133](0133-font-files-under-the-open-font-license.md), the stage's split in ADR-0131.

## Decision

**The engine writes its own framework, on building blocks.** It adopts no framework and forks none;
ADR-0098's decision stands. parley, taffy and AccessKit do the hard and general parts; the engine
does its rendering, its focus and layers, its documents and what games add. As principle 7 asks,
they are wrapped at the engine's boundary: parley's and taffy's types stay inside `slime-ui`, and
AccessKit's inside `slime-ui` and `slime-platform`, how its tree crosses between them decided by the
milestone that builds the adapter.

**A retained tree of the framework's own.** The interface is a tree of elements that lasts from one
frame to the next, in an arena of the framework's: a handle of an index and a generation, a key
unique among its siblings, given by whoever builds it, and an element's path of keys, which names it
for agents, tests and saved layouts. Its elements are not entities of a world: the game's world is
read only while the presentation extracts
([ADR-0121](0121-a-game-as-a-simulation-and-a-presentation.md)), saved and later replicated, while
the interface changes many times a frame, runs at real time while game time pauses
([ADR-0107](0107-the-execution-model.md)), and is never saved; a world of its own would give each
element the weight of an entity, a transform of three dimensions in `f64`, an identity of strings, a
place in saves. Each element carries what changed in it, its style, layout, paint or meaning, from
the milestone that builds the tree, so that a frame redoes only what changed.

**Built by code, when what it reads changes.** The editor's panels and the overlay are Rust code.
A panel builds its elements again only when a revision that it reads moves: the world's ticks and
its structure ([ADR-0119](0119-the-worlds-ticks-blocks-and-log.md)), or a counter of the editor's;
the elements built again are matched with those before by their keys, so that focus, scrolling,
text being edited and animations survive. There is no graph of signals, nor a property read again
at every frame.

**Actions become commands.** A person's input becomes an *action* of the element it reaches, a
click, a value set, a node expanded, or a navigation intent, next, previous, a direction, accept,
cancel; an agent and a screen reader send the same actions, which reach the same handler. What
changes the world leaves the interface as a command, the editor's
([ADR-0109](0109-stable-references-and-undo.md)) or a game's, applied at the start of the next
frame: what a person does, an agent does (principle 10). What changes only the interface, a scroll,
a tab, a node expanded, stays in its tree. Input goes to the top layer of the interface that takes
it, popups and modals above panels; focus has scopes, Tab order and directions; shortcuts are data
that name commands.

**The tree says what it means, to agents and to screen readers.** Each element that a person reads
or uses has a role, a name, a value, states, the actions it takes, its bounds and its path, kept up
to date from the milestone that builds the tree. Agents read it and act on it through the
development channel ([ADR-0122](0122-the-development-channel.md)) and the MCP server, with the
captures that exist; a test does the same without a window. Screen readers read it through
AccessKit's adapters in `slime-platform`, behind a feature built into the editor and the examples,
and left out of games until Stage 7.

**Styles are typed, themes are documents.** An element's style is a set of typed properties, grouped
by what they make again. Style sheets and themes are documents of reflected types, read and checked
by `slime-serialize` ([ADR-0080](0080-reflection-and-serialization.md)), with named values that
rules share, and rules selected by an element's kind, classes, key and states; cooked and loaded
again while a program runs; matched again only when an element's classes, state or place change.

**Text in every writing system.** parley shapes, orders, breaks and lays out text, and edits it,
input methods included, with fontique finding fonts: its `complex-scripts` feature on, and
fontconfig loaded at run time rather than linked. swash rasterizes glyphs into atlas pages. Fonts
are always named in a font's stack, never left to fallback alone, which the measurements make a
rule. Games ship their fonts as assets; the editor and the overlay embed theirs, and the editor
falls back on the system's for other scripts, finding the system's font for a script once, then
naming it in its stacks ([ADR-0133](0133-font-files-under-the-open-font-license.md)). A glyph's key
holds its representation, coverage for now, so that distance fields come as another. Shaped text is
kept in a cache within a budget.

**Layout by taffy, on the framework's tree.** taffy's flexbox and grid run through its traits on the
framework's own arena, so that there is one tree; a list, a tree view or a dock lays its children
out itself. Lists and trees build only the rows that show.

**Three crates; the drawing in the renderer.**

- `slime-ui`, headless: the tree, styles, layout, text, input, focus, the tree's meaning, and a
  *display list*, owned data of what to draw, `Send + 'static` as a frame is
  ([ADR-0111](0111-the-frame-contract.md)). Text is a module of it until a second user asks for a
  crate of its own.
- `slime-render` gains a *canvas*: rounded rectangles with their borders and shadows, glyphs and
  images, each clipped, drawn in a few batches after the views, into any target. It shares the
  renderer's shader reloading ([ADR-0077](0077-hot-reload-of-shaders.md)), its GPU timers, its
  recovery from a lost device
  ([ADR-0103](0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md)) and the order of its
  passes. The interface's images are cooked textures that the renderer's asset cache holds, once its
  requests, which name meshes today ([ADR-0128](0128-asset-requests.md)), name images too, as the
  milestone that draws them decides.
- `slime-ui-render`, a bridge, as `slime-world-render` is between the world and the renderer: it
  turns a display list into a canvas, and keeps the glyphs' atlas pages within a budget, the least
  recently used glyphs leaving first, rasterized again after a lost device.

`slime-gfx` gains what it lacks, each piece with its first user: a texture of one channel, writing a
part of a texture, which [ADR-0129](0129-drawing-for-the-editor.md) left to the interface, and a
scissor apart from the viewport.

**A viewport is a view drawn into its own target**, which the canvas samples as an image, as
ADR-0129 already lets another pipeline sample a view. Drawing a view into a rectangle of the
window ([ADR-0126](0126-views-apart-from-the-renderer.md)) stays, for a game's view under its HUD.

**Color.** The interface is drawn in display space, after tone mapping. Whether it blends in
linear space or in sRGB's encoding, as design tools do, is chosen at the milestone that draws it,
from images that the owner judges.

**What the platform gains**, inside `slime-platform`, winit's types and the other crates' staying
there: input methods, the clipboard, cursor shapes, the raw motion of the mouse, dialogs to open and
save, files dropped where winit gives them, and AccessKit's adapter. winit moves to 0.31 once it is
released, for files dropped on Wayland; until then, the editor imports through its dialogs.

**Shaped for the games from the first line.** What Stage 7 adds for games is added, not written
again, because these exist from the framework's first milestones: in the display list, each
instance's transform, clip and layer, though the transform stays the identity, and the layer the
root's, until Stage 7; roots that are not a window; start and end rather than left and right, and a
direction per root; every text shown is a value that can become a key of a translation table;
actions and intents, to which gamepads will be mapped; elements whose properties are reflected. Most
have a first user in Stage 6b: clips in every scrolled panel, text laid out from right to left in
the tests, Tab and arrow keys, themes. The transforms and the layers have none before Stage 7: the
owner chose on 2026-10-22 to give them their place now, as data, so that Stage 7 builds them without
changing the display list and the canvas's shader, against principle 3's rule of a second use, which
they would otherwise wait for. Stage 7 builds animation and transitions, transforms, layers with
masks, effects on text through distance fields, UI documents, UI on surfaces in the world, and focus
driven by gamepads.

**Authoring.** The editor's panels and the overlay are written in Rust. UI documents, the games'
screens as data, come at Stage 7, their notation decided then.

**The dev overlay is the framework's first client.** It is rewritten on `slime-ui` as soon as the
framework's base widgets are ready, before the editor's panels: its statistics, time controls, world
panel and reflected value editors, which the editor's inspector then reuses. Its world panel lays
out only what shows (G46), and its fields change a value only when the user changed it (G33). egui,
egui-wgpu and the exception for their fonts leave the engine then, which meets ADR-0052's condition:
something does at least as well, on a checklist written at that milestone. `slime-gfx`'s `wgpu`
feature keeps its other users, the engine's own tests ([ADR-0031](0031-dev-overlay-with-egui.md)).

**Dependencies**, each added at the milestone that first uses it, its version and what it adds to
CI's build checked then, each recorded in [`docs/dependencies.md`](../dependencies.md):

| Crate | License | For |
|---|---|---|
| parley 0.12, fontique 0.12 | Apache-2.0 OR MIT | Text: shaping, bidi, line breaking, fallback, editing (brings ICU4X, Unicode-3.0) |
| swash 0.2 | Apache-2.0 OR MIT | Glyph rasterization; replaced by the engine's own on zeno if its bound on Skrifa holds parley back |
| etagere 0.3 | Apache-2.0 OR MIT | Places glyphs in atlas pages, and frees them one by one |
| taffy 0.15 | MIT | Flexbox and grid |
| accesskit 0.25 | Apache-2.0 OR MIT | The tree for screen readers |
| accesskit_winit 0.34 | Apache-2.0 | Its adapter for winit's windows, behind a feature |
| arboard 3.7 | Apache-2.0 OR MIT | The clipboard, with its Wayland feature |
| rfd 0.17 | MIT | Dialogs: the desktop portal on Linux, through D-Bus loaded at run time; Windows' own |

## Alternatives considered

Where elements live, and how the tree is built:

- **Entities of a second world, the interface's own**, as Godot's nodes in scenes: scenes, prefabs,
  the inspector, undo and hot reload would serve the interface as they are; but each element would
  carry an entity's weight and the simulation's machinery, a world's typed storage holds no layout
  or text caches nor closures, and the reactivity that Bevy is still building would be needed.
- **Entities of the game's world**, as Bevy does: the presentation would write the world it may only
  read, and saves and the network would see the interface.
- **Immediate mode**, as egui: simple to write, but no lasting identity for agents, animations or
  documents, everything laid out again at each frame, and Unity left it for its editor.
- **Signals**, or **whole views compared** as Xilem does: a second record of what changed beside the
  world's ticks, or the cost of comparing.

Adopting or forking:

- **egui for the editor, the engine's own for games**: rejected by ADR-0098, two ways to build an
  interface for years.
- **A fork of Masonry**, the closest: some 30 000 lines that move fast, drawn through Vello, which
  would have to be replaced, and three or four versions behind parley and Vello.
- **Slint**: refused by ADR-0006.

Building blocks:

- **cosmic-text**: slower on Chinese and emoji, Thai broken inside its words, and `ttf-parser`.
- **The engine's own shaping on HarfRust and ICU4X**: thousands of lines that parley already holds.
- **The engine's own rasterizer on zeno**, about 300 lines: no bound on Skrifa, but color emoji
  still to write. It stays the fallback if swash holds parley back.
- **vello_cpu** as the rasterizer, which egui 0.36 uses: color glyphs and paths too, but a young
  crate that leaves the build with egui, where swash was measured.
- **The engine's own layout**: about 2 000 lines for flexbox alone, without grid.
- **window_clipboard**, or **smithay-clipboard** beside arboard, which use the window's own Wayland
  connection: window_clipboard's `connect` is an `unsafe` function, which the engine's code does not
  call, and smithay-clipboard takes the connection's raw pointer.
- **Text as a crate of its own, `slime-text`**, from the start, which two proposals made: text apart
  from the interface, for a later user such as names drawn over characters in the world; but no such
  user in Stage 6b.

Drawing:

- **A crate of its own on `slime-gfx`**, beside the renderer, as Unreal's Slate draws beside its
  scene renderer, which three of the study's four proposals made: the scene renderer stays about
  scenes; but shader reloading, GPU timers and the recovery from a lost device made a second time
  or shared, the interface's images through the renderer's cache all the same, and two ways to draw.
  Godot and Bevy draw their interfaces through their renderers.
- **Vello**: compute shaders, which `slime-gfx` gives only at Stage 9a, and wgpu's types.

Screen readers:

- **At Stage 7, with the games' menus**: Stage 6b lighter by D-Bus's crates and a check by hand;
  but the editor of 0.7 unreadable by screen readers, the tree that agents read never checked
  against a real reader before Stage 7, and the adapter's hold on how windows are made added
  afterwards.

egui:

- **Removed after the editor's inspector**, with the value editors that it builds: the editor
  first, but two ways to build an interface for longer, and the framework tried in a game later.
- **Removed at Stage 7**, with the games' UI: no risk for the overlay in Stage 6b, but two stacks
  for a year, the world panel slow until then, and wgpu's upgrades still tied to egui.

## Consequences

- Stage 6b's milestones build the framework in the order of ADR-0131: the second adds `slime-ui`,
  the third the canvas and `slime-ui-render`, each recording its design in an ADR of its own.
- `slime-render` draws in two dimensions too: the interface's canvas, beside the views.
- The engine has one way to build an interface from the eighth milestone of Stage 6b on; egui's
  versions no longer hold wgpu's back, and `deny.toml` loses its exception for egui's fonts.
- Agents read and drive the interface of the overlay, then of the editor, as they read and drive a
  game's world.
- About 40 crates enter the build, ICU4X's most of them, and 46 more on Linux where the
  accessibility feature is on; about 20 leave with egui.
- Text quality, input methods on Wayland and on Windows, the clipboard on Wayland's compositors,
  and screen readers are checked by hand by the owner, each at its milestone.
- [`docs/ROADMAP.md`](../ROADMAP.md) lists the framework's design among the fixed decisions.
