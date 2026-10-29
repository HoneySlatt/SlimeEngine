# Architecture

This document maps the code: which crates exist, what each one is responsible for, how they
depend on each other, and where to start reading. It describes the repository as it is now;
[`ROADMAP.md`](ROADMAP.md) describes where it is going, and [`adr/`](adr/README.md) records why
it is built this way.

## Repository layout

```text
.
├── .github/workflows/   CI: scripts/check.sh on Linux, in three parts; tests and samples on Windows
├── benches/             the engine's benchmarks (`slime-benches`), whose figures docs/ keeps
├── Cargo.toml           virtual workspace: shared metadata, lints and dependency versions
├── Cargo.lock           the exact version of every dependency, checked in
├── crates/              engine crates, one directory per crate, named after the package
│   ├── slime-app/       a game run as a simulation and a presentation by a host: the main loop
│   ├── slime-assets/    assets as games load them: asset IDs, cooked data, the manifest
│   ├── slime-channel/   the development channel: a running game driven by tools, its protocol
│   ├── slime-color/     the color type of every API: linear colors, the sRGB transfer function
│   ├── slime-cook/      the asset pipeline's cooker, a library and its program
│   ├── slime-devui/     the dev overlay, on the engine's interface: tooling
│   ├── slime-editor/    the editor: its core, its operations, and its window
│   ├── slime-gfx/       GPU layer (wgpu)
│   ├── slime-input/     the vocabulary of input: keys, mouse buttons, modifiers, the wheel
│   ├── slime-mcp/       the engine's MCP server: an agent drives a running game through it
│   ├── slime-platform/  window, its events and input
│   ├── slime-reflect/   reflection: types described at run time, their values read and set by path
│   ├── slime-reflect-derive/  the derive macro of reflection, which slime-reflect re-exports
│   ├── slime-serialize/ serialization: reflected values in TOML documents, or a binary encoding
│   ├── slime-render/    renderer: meshes, PBR materials, cameras, views, lights, shadows, HDR, tone mapping
│   ├── slime-tasks/     the engine's task pool: the threads on which the engine computes
│   ├── slime-ui/        the UI framework, without a GPU: its tree of elements, widgets, text,
│   │                    display list, and the editor's look (look/: fonts and icons, a style
│   │                    sheet, five themes)
│   ├── slime-ui-render/ the bridge from a display list to the renderer's canvas, with the glyphs' atlas
│   ├── slime-world/     the world: a game's entities, their components and their hierarchy
│   └── slime-world-render/  the bridge from a world to the renderer: what a world draws, each frame
├── deny.toml            dependency license, advisory and source rules
├── docs/                roadmap, plan, decisions (adr/), glossary, dependencies, measurements,
│                        this file
├── examples/            runnable examples, one per feature, with their tests (`slime-examples`)
├── flake.nix            Nix development shell
├── flake.lock           pinned inputs of the development shell
├── games/               games made with the engine through its public API
│   ├── hunnawood/       Hunna Wood, a role-playing game: the sample every stage grows
│   └── hunnawood-editor/ Hunna Wood's editor: the engine's editor, which knows the game's types
├── rust-toolchain.toml  pinned Rust toolchain
├── scripts/             check.sh, every automated check as CI runs them; measure.sh, the build's
│                        and the benchmarks' figures; the scripts that fetch DXC and the sample
│                        game's source assets, and write their credits; subset-fonts.sh, the
│                        fonts of slime-ui's tests; screen-reader-check.sh, an interface read
│                        through AT-SPI as a screen reader does; cooker-dependencies.sh, CI's
│                        key of the cooked zone
├── AGENTS.md            rules for every contributor
├── CHANGELOG.md         notable changes, per release
├── CONTRIBUTING.md      environment, checks, how a change is made and reviewed
├── LICENSE              MPL-2.0
└── README.md            what the project is, its state, where to read next
```

[ADR-0004](adr/0004-workspace-layout-and-crate-naming.md) sets the layout and naming rules: the
root is a virtual workspace, engine crates are `crates/slime-<role>/`, games are
`games/<name>/` ([ADR-0055](adr/0055-games-in-the-workspace.md)), and a directory or a crate is
created only when code needs it.

## Crates

| Crate | Role | Depends on | Status |
|---|---|---|---|
| [`slime-input`](../crates/slime-input/README.md) | The engine's vocabulary of input, without a window: keys by position and by meaning, mouse buttons, modifiers, the wheel ([ADR-0142](adr/0142-input-and-focus.md)) | none | The types of a key's press and release, the modifiers, the mouse's buttons and the wheel, what an input method does, and the pointer's shapes ([ADR-0145](adr/0145-text-edited.md)); the names of the keys, listed once |
| [`slime-platform`](../crates/slime-platform/README.md) | The window, its events and keyboard and mouse input, and that input as data, the keys and buttons held; the only crate that uses the windowing library (winit), the clipboard's (arboard), the dialogs' (rfd) and AccessKit's adapter | `slime-input`, `winit`, `arboard`, `rfd`, `raw-window-handle`, `thiserror`, `tracing`; with its `accessibility` feature, `accesskit`, `accesskit_winit` | One window; window, keyboard and mouse events, and the text that input methods compose; the keys and mouse buttons held, pressed and released since the last frame; the input method allowed and placed, the pointer's shape, and the system's clipboard of text ([ADR-0145](adr/0145-text-edited.md)); the scheme of colors that the system prefers, where winit tells it ([ADR-0149](adr/0149-the-looks-foundations.md)); the cursor held, hidden, and the mouse's raw motion meanwhile, and the system's dialog to choose a file, on a thread of its own ([ADR-0157](adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md)); with its `accessibility` feature, AccessKit's adapter of the window, made before it shows, the requests of screen readers as events and the updates of their tree ([ADR-0146](adr/0146-screen-readers.md)) |
| [`slime-app`](../crates/slime-app/README.md) | A game run as a simulation, its rules, and a presentation, what the player sees, by a host that owns its world, real time and game time, its input and its commands, through the phases of each frame ([ADR-0107](adr/0107-the-execution-model.md), [ADR-0121](adr/0121-a-game-as-a-simulation-and-a-presentation.md)); the main loop, diagnostics setup, headless mode | `slime-platform`, `slime-gfx`, `slime-world`, `slime-reflect`, `slime-ui`, `ctrlc`, `thiserror`, `tracing`, `tracing-subscriber`, `nu-ansi-term` (Windows only); with its `development` feature, `slime-channel`, `slime-serialize`; with its `accessibility` feature, `accesskit` | Headless runs, and windowed runs drawn by the GPU, which go on with a new device after a lost one when the game asks; game time that pauses, slows down and steps, and that a game holds while it is not ready; commands applied at the start of a frame; a host that a test drives without a GPU; with its `development` feature, which a game turns on by default, the development channel, through which tools drive a run started with a token ([ADR-0122](adr/0122-the-development-channel.md)), and read and change the settings of the views that a presentation names, and read its reports, described by reflection ([ADR-0127](adr/0127-the-settings-of-a-view.md), [ADR-0128](adr/0128-asset-requests.md)), and ask it to pick the entity at a pixel of a view ([ADR-0130](adr/0130-picking.md)), and read its interface, act on it and capture its elements ([ADR-0143](adr/0143-what-agents-see-of-the-interface.md)); a presentation's interface connected to its window, given its events first, which it takes before the game when they reach its elements, the system's clipboard, the pointer's shape and the input method ([ADR-0145](adr/0145-text-edited.md)), and each frame's time, by which its transitions go on ([ADR-0149](adr/0149-the-looks-foundations.md)); with its `accessibility` feature, which the examples turn on, the interface's meaning translated into the tree that screen readers read, and their requests given back to it ([ADR-0146](adr/0146-screen-readers.md)); the commands that a host receives applied through its simulation, and an editor's requests answered by it, kept while it says that they wait ([ADR-0161](adr/0161-the-open-step-and-the-handles.md)); frames run only when something asks for one, with a window or without, as the editor's program asks, and a waker of the loop for other threads ([ADR-0156](adr/0156-the-editors-window-and-its-operations.md)); the user's data and state directories, and a file that one program at a time holds ([ADR-0135](adr/0135-the-editors-program.md)) |
| [`slime-gfx`](../crates/slime-gfx/README.md) | The GPU layer; the only crate that uses wgpu, the engine's own tests reaching the device through its `wgpu` feature | `wgpu`, `pollster`, `raw-window-handle`, `slime-color`, `thiserror`, `tracing` | Opens a GPU device, and another in place of a lost one, each with an identity that its objects carry, an object used with another device panicking ([ADR-0157](adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md)); draws into offscreen images, 8-bit or 16-bit float, and reads them back; draws into a window; later passes sample what earlier ones drew; colors laid over a target by their alpha, straight or already multiplied by it (blending); an sRGB target, a window's image included, drawn into without its encoding ([ADR-0137](adr/0137-the-canvas.md)); targets of integers, read back without waiting ([ADR-0130](adr/0130-picking.md)); depth-only passes; images of four samples per pixel, resolved for anti-aliasing, their depth read sample by sample ([ADR-0159](adr/0159-picking-and-the-selection.md)); textures and cube textures whose levels the caller gives, one channel of 8 bits, half-precision floats and BC7 and BC5 blocks among their formats; storage buffers, buffers written by ranges, instanced draws from a base vertex; an uploader that submits the copies of what loads bring within a budget of bytes per frame, on a thread of its own, the lowest rank first; a window's image read back where the system allows it |
| [`slime-channel`](../crates/slime-channel/README.md) | The development channel: the protocol between a running game and the tools that drive it, and both of its ends, the game's listener and the tools' client ([ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md), [ADR-0122](adr/0122-the-development-channel.md)); headless | `getrandom`, `slime-reflect`, `slime-serialize`, `thiserror`, `tracing` | A game started with a token listens on the machine's own address and accepts only the token and its version; the host's controls, the world read and changed through reflection, scripted keys, captures of the game's image or of one of its views ([ADR-0126](adr/0126-views-apart-from-the-renderer.md)), the settings of its views read and changed ([ADR-0127](adr/0127-the-settings-of-a-view.md)), its reports read ([ADR-0128](adr/0128-asset-requests.md)), the entity at a pixel picked ([ADR-0130](adr/0130-picking.md)), the elements of its interface listed by role and name, found at a pixel, acted on, a field given a value, and captured ([ADR-0143](adr/0143-what-agents-see-of-the-interface.md), [ADR-0146](adr/0146-screen-readers.md)), its state, log and frames' times, and its run saved to a file and loaded from one ([ADR-0125](adr/0125-saves-per-scene.md)); a game started by a tool with a new token; an editor's requests, steps of commands, undo and redo, its history, its journal, its selection and its level written back ([ADR-0135](adr/0135-the-editors-program.md)) |
| [`slime-color`](../crates/slime-color/README.md) | The color type of every API of the engine, `LinearColor`, and the sRGB transfer function, below every crate that holds a color ([ADR-0045](adr/0045-one-linear-color-type.md), [ADR-0079](adr/0079-the-engines-own-world.md)); headless | `slime-reflect` | Linear colors and their constructors, sRGB bytes decoded and encoded, linear values encoded; colors in OKLCH, and brought back into sRGB's gamut ([ADR-0149](adr/0149-the-looks-foundations.md)) |
| [`slime-tasks`](../crates/slime-tasks/README.md) | The engine's task pool: the threads on which the engine computes, with a priority for the work that a frame waits for ([ADR-0107](adr/0107-the-execution-model.md)); headless, on the standard library alone | none | One pool, the machine's logical cores but one; frame and background tasks, a thread always left for frame tasks; items computed in parallel, the calling thread taking its share |
| [`slime-ui`](../crates/slime-ui/README.md) | The UI framework, for the editor and the interface of games ([ADR-0132](adr/0132-the-engines-ui-framework.md)); today its tree of elements and their layout ([ADR-0140](adr/0140-the-tree-of-elements.md)), its style sheets and themes ([ADR-0141](adr/0141-style-sheets-and-themes.md)), its input and focus ([ADR-0142](adr/0142-input-and-focus.md)), its fields ([ADR-0145](adr/0145-text-edited.md)), its widgets and the editor's look ([ADR-0148](adr/0148-widgets.md)), its layers ([ADR-0150](adr/0150-layers.md)), what its tree means to agents ([ADR-0143](adr/0143-what-agents-see-of-the-interface.md)), its text ([ADR-0136](adr/0136-the-interfaces-text.md)) and its display list ([ADR-0139](adr/0139-text-drawn-and-the-display-list.md)); headless | `slime-color`, `slime-reflect`, `slime-serialize`, `slime-input`, `parley`, `fontique`, `swash`, `taffy`, `icu_properties`, `thiserror`, `tracing` | Fonts registered from their bytes and named in stacks, the system's for a program that asks for them, found by fallback once for each writing system then named for it; text shaped, put in the order of its directions, broken into lines and aligned in every writing system, cut with an ellipsis to a box of a width and a most lines; its lines, its glyph runs, the cluster under a point, the position nearest it, carets and selections; each glyph placed at a whole pixel and a position between two, its image rasterized as coverage or color, without hinting; laid-out text kept within a budget of memory, shared with the display lists that hold it; a display list of boxes, shadows, gradients, texts and images, each clipped, its images named by keys; a tree of elements kept from frame to frame in an arena, built by code when a revision moves, matched by keys, its roots each of a size, a scale and a direction; boxes, labels, images, buttons that show their states, boxes that scroll and lists that build only the rows that show; typed styles in logical pixels, with start and end sides; flexbox and grid by taffy, mirrored for a root from right to left; a frame that lays out and draws again only what changed; style sheets of rules that select elements by kind, classes, key, states and ancestor and set a part of their style, themes of the values that the rules name, the element's own style over them; a person's input and an agent's actions turned into actions in a queue, the focus moved by Tab and by the arrows within its scope, popups and modal roots stacked over a window, shortcuts read from tables that name commands, and the mouse's buttons that a program reads from them ([ADR-0142](adr/0142-input-and-focus.md), [ADR-0160](adr/0160-two-viewports-and-blenders-way.md)); fields of text, of one line or several, and of numbers, typed, selected by the keys and the pointer, undone, copied and pasted, composed through input methods, which the program reads as actions, changed and committed ([ADR-0145](adr/0145-text-edited.md)); the tree's meaning, made when asked, each element's role, name, value, states, actions, bounds and path ([ADR-0143](adr/0143-what-agents-see-of-the-interface.md)), and what changed in it, each element marked as its meaning changes, named by a number never given again, with a field's text by runs ([ADR-0146](adr/0146-screen-readers.md)); check boxes, sliders, sections that fold, bars of tabs, two panels and the handle between them, and scroll bars, each an element kind that keeps its state in the tree, takes the pointer, the keys and the focus, and means something to agents and screen readers, its parts styled apart from its box; with its `editor-look` feature, which the examples turn on, the editor's look, the fonts Geist and Geist Mono, Lucide's icons, and a style sheet and stacked themes, a base, dark, light, high contrast and compact ([ADR-0148](adr/0148-widgets.md), [ADR-0149](adr/0149-the-looks-foundations.md)); themes whose values name others and compute colors in OKLCH, stacked; transitions of colors, opacity and a section's height that rules declare, over the time that the program gives; an opacity that an element's descendants share; the focus shown to the keyboard alone; boxes on whole pixels; OpenType features of a text ([ADR-0149](adr/0149-the-looks-foundations.md)); layers that the interface places beside an element, at a point or floating, kept within the window and closed as the user leaves them, a modal one dimming the window below it by its backdrop ([ADR-0157](adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md)); menus, their submenus and a bar of menus, moved through by the keys; context menus asked for by the pointer's second button and Shift+F10; lists to choose from; tooltips once the pointer rests; floating panels moved, sized, folded and raised; glyphs of a font of icons in place of controls' shapes ([ADR-0150](adr/0150-layers.md)); surfaces that show an image that the program draws and give it the pointer ([ADR-0158](adr/0158-the-viewport.md)); placeholders, buttons that toggle, controls of segments, bars of progress, tabs' counts, fields of search, a field's prefix that drags its number, expressions and units in fields of numbers, the box of a section's header, tooltips of a title and a text, notifications, the palette of commands and the sheet of shortcuts ([ADR-0151](adr/0151-rich-widgets.md)); color wells and their pickers ([ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)) |
| [`slime-ui-render`](../crates/slime-ui-render/README.md) | The bridge between the interface and the renderer, as `slime-world-render` is between the world and the renderer ([ADR-0132](adr/0132-the-engines-ui-framework.md), [ADR-0139](adr/0139-text-drawn-and-the-display-list.md)) | `slime-ui`, `slime-render`, `slime-gfx`, `etagere`, `tracing` | A display list drawn through the renderer's canvas; the glyphs' atlas, pages of coverage and of colors in which etagere places each glyph's image, rasterized once, within a budget, the glyphs that no draw asked for the longest leaving first, again after a lost device; images given by the program for their keys |
| [`slime-render`](../crates/slime-render/README.md) | The renderer: draws instances of meshes with their materials, lit by directional, point and spot lights, seen from cameras through views, into render targets or rectangles of them, and reports what each view cost; built on `slime-gfx`'s types, without wgpu | `slime-gfx`, `slime-assets`, `slime-color`, `slime-reflect`, `slime-tasks`, `glam`, `thiserror`, `tracing` | Textures and scenes cooked by the asset pipeline, loaded onto the GPU, BC7 and BC5 among them, normal textures read by x and y; meshes with tangents, simple shapes made in code (a cube, a sphere, a square), glTF's metallic-roughness materials (five textures, masks, double-sided, values that change while a game runs, found by name in a loaded scene), unlit materials, their color shown as it is, masked or blended ([ADR-0129](adr/0129-drawing-for-the-editor.md)), cameras in perspective or orthographic, drawing their whole image or a part of it, several views a frame sharing the pipelines, the meshes and the shadow atlases, each with its own targets at the size it draws, made again once a new size holds still, and its settings: a render scale, no anti-aliasing or four samples, a mode of debugging that shows normals, shadows, the base color or the light alone, and the shadows, the sky or the bloom left out ([ADR-0126](adr/0126-views-apart-from-the-renderer.md), [ADR-0127](adr/0127-the-settings-of-a-view.md)), the depth test, up to 16 directional, point and spot lights in physical units (`KHR_lights_punctual`), a procedural sky computed on the CPU that surfaces reflect (image-based lighting) and that is drawn behind them with its sun's disc, the shadows of the first directional light that casts them (four cascades in one atlas, filtered over 5 × 5 texels) and of every spot light that casts them (a tile each of one atlas), mirrored instances drawn with their front faces, the scene drawn in HDR with its light exposed, at four samples per pixel (multisample anti-aliasing) unless a view draws without, an energy-conserving bloom, then tone mapped, the passes ordered by a render graph and recorded sixteen to a submission, a whole frame in one, statistics with the GPU time of each pass; scenes loaded in the background, and again when their cooked data changed, keeping what did not change; the meshes a game draws kept by asset ID in a cache, asked for by requests of a priority whose handles hold them, loaded in batches by class, each mesh that fails alone, kept within budgets of the textures' and the meshes' memory once nothing holds them, loaded again when their cooked data changes and, through the requests given back, after a lost device ([ADR-0084](adr/0084-what-the-world-draws.md), [ADR-0128](adr/0128-asset-requests.md)); what a client draws into a view besides the scene, lines a width in pixels wide and instances, in the scene and over the image ([ADR-0129](adr/0129-drawing-for-the-editor.md)), and an outline around instances, dimmed where the scene hides them ([ADR-0159](adr/0159-picking-and-the-selection.md)); the number of the instance drawn at each pixel of an area of a view, picked a frame or more later ([ADR-0130](adr/0130-picking.md)); an interface's canvas, boxes with rounded corners and borders, their shadows, boxes filled with a gradient and images, each clipped, drawn over any of its targets, its colors mixed as sRGB values ([ADR-0137](adr/0137-the-canvas.md), [ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)); its shaders replaced while it runs |
| [`slime-reflect`](../crates/slime-reflect/README.md) | Reflection: types described at run time, their values as plain data, read and changed by property path through a registry, compared and their differences applied ([ADR-0080](adr/0080-reflection-and-serialization.md), [ADR-0085](adr/0085-reflection.md)); headless | `glam`, `slime-reflect-derive` | `Reflect` and its derive, with stable names, aliases, versions and their upgrades, default values, skipped and transient fields, ranges and units; one value type, references to assets and entities kinds of their own; descriptions of types; a registry, explicit and recursive, that holds types with no Rust type behind them, checks values, sets their parts by path within their fields' ranges and lists their leaves; the standard types, glam's `Vec3`, `DVec3` and `Quat`; no files: `slime-serialize` writes and reads them |
| [`slime-reflect-derive`](../crates/slime-reflect-derive/README.md) | The derive macro of reflection, `#[derive(Reflect)]`, which `slime-reflect` re-exports | `proc-macro2`, `quote`, `syn` | Structs and enums, their doc comments and attributes; generic types, lifetimes, unions and types without a name refused |
| [`slime-serialize`](../crates/slime-serialize/README.md) | Serialization: values of reflected types written to TOML documents by the engine's own writer and read back, from earlier versions of their types too, and the same documents in a binary encoding of the engine's own ([ADR-0080](adr/0080-reflection-and-serialization.md), [ADR-0086](adr/0086-serialization.md), [ADR-0094](adr/0094-the-binary-encoding-of-documents.md)); headless | `slime-reflect`, `toml_edit`, `thiserror` | Documents of any registered type's value, whole or in part; the same bytes for the same value; versions written past 1 and earlier values brought forward by their types' upgrades; strict and lenient reading, errors by line, column and path, limits on size and depth; documents of several values, references to entities written as the names that a world gives them, and to assets as a document names them, their IDs unless it says otherwise; a document's layout read and its errors placed by the program that reads it, as a world reads scene files; the binary encoding written from a text and read through the same reader, its errors by byte; the comments of a document written by hand kept when a program writes it again ([ADR-0134](adr/0134-the-editors-core.md)) |
| [`slime-world`](../crates/slime-world/README.md) | The world: a game's entities, their components and their hierarchy, kept by the engine ([ADR-0079](adr/0079-the-engines-own-world.md)); headless | `slime-assets`, `slime-color`, `slime-reflect`, `slime-serialize`, `glam`, `thiserror`, `tracing` | Entities as handles of an index and a generation, the components attached to them stored by type, queries that read through an iterator or change through a closure ([ADR-0082](adr/0082-the-worlds-storage-and-handles.md)); every change stamped with the world's tick, written through guards, so that readers ask what was added, changed or removed since they last read, and whether the hierarchy moved ([ADR-0108](adr/0108-what-the-world-knows-of-its-changes.md), [ADR-0119](adr/0119-the-worlds-ticks-blocks-and-log.md)); parents and children, transforms with `f64` translations, a uniform scale and a stretch, and where each entity is in the world, always current ([ADR-0083](adr/0083-transforms-and-the-hierarchy.md)); scenes spawned as entities with persistent identities, glTF scenes and scene files whose placements of prefabs carry overrides, resolved by a library of scenes, and written back as scene files that keep their placements ([ADR-0089](adr/0089-scenes-and-prefabs.md), [ADR-0091](adr/0091-scene-documents-and-placements.md), [ADR-0092](adr/0092-a-world-written-back-as-a-scene.md)); what is drawn, meshes by asset ID, lights and hidden subtrees, and the transforms before the last fixed step of what moved during it, with the ancestor that moved which carries each entity that did not ([ADR-0084](adr/0084-what-the-world-draws.md), [ADR-0168](adr/0168-rearranging-the-hierarchy.md)); every component reflected, and listed, read and set by name and property path through the world's registry ([ADR-0085](adr/0085-reflection.md)); saves of what a run changed since the game settled the world, at the cost of what changed, the scenes that it spawned saved by their keys and assets, loaded back, what a run changed of a scene unloaded kept until it is spawned again, and entities adopted out of their scenes under keys of their own ([ADR-0087](adr/0087-the-worlds-documents.md), [ADR-0093](adr/0093-saves-and-hot-reload-with-placements.md), [ADR-0125](adr/0125-saves-per-scene.md)); components marked driven, which saves and scene files written back leave as their author wrote them ([ADR-0124](adr/0124-driven-components-keys-and-created-entities-in-levels.md)); commands that name their entities by persistent identity, applied by a host at the start of a frame, each giving back the command that undoes it and written as text, a deletion recorded and restored under the same identities, entities created under an identity of a scene and children put at a rank ([ADR-0121](adr/0121-a-game-as-a-simulation-and-a-presentation.md), [ADR-0134](adr/0134-the-editors-core.md)); the commands that create subtrees again as they are, read under new identities ([ADR-0162](adr/0162-creating-copying-and-arranging.md)) |
| [`slime-world-render`](../crates/slime-world-render/README.md) | The bridge from a world to the renderer: builds, at each frame, the instances, the lights and the sky's environment that a world draws, from the GPU objects of an asset cache ([ADR-0084](adr/0084-what-the-world-draws.md)); neither the world nor the renderer depends on it | `slime-world`, `slime-render`, `slime-gfx`, `slime-assets`, `tracing` | Meshes drawn where the world draws them between fixed steps, those the cache does not hold asked for, at priority now, a handle of each mesh named held ([ADR-0128](adr/0128-asset-requests.md)); lights at their entities, each named by its entity in the frame; hidden subtrees left out; the game's sky around the world's sun, given once; each instance numbered with its entity's handle, which picking by the GPU gives back, and picking by the CPU, a ray against the meshes that a world places, read from cooked data, the entity it meets first or every one, the nearest first, and the box in the world of an entity's meshes ([ADR-0130](adr/0130-picking.md), [ADR-0159](adr/0159-picking-and-the-selection.md), [ADR-0161](adr/0161-the-open-step-and-the-handles.md)) |
| [`slime-assets`](../crates/slime-assets/README.md) | The assets as games load them: asset IDs, cooked data and its manifest, read by ID from a directory of cooked data ([ADR-0069](adr/0069-the-asset-pipeline.md)); headless | `slime-reflect`, `ktx2`, `thiserror` | The asset ID and its text form, a UUID; cooked textures in KTX2 files ([ADR-0071](adr/0071-cooked-textures.md)), meshes, materials and scenes, their nodes' extras and typed components among them; cooked scene files, style sheets, themes and tables of shortcuts, read as bytes ([ADR-0095](adr/0095-cooked-scene-files.md), [ADR-0141](adr/0141-style-sheets-and-themes.md), [ADR-0142](adr/0142-input-and-focus.md)); the manifest; reading them by ID; a watch of the manifest, for hot reload |
| [`slime-cook`](../crates/slime-cook/README.md) | The cooker: turns source assets into cooked data before games run; a library and its program, which no game links ([ADR-0069](adr/0069-the-asset-pipeline.md)) | `slime-assets`, `slime-color`, `slime-reflect`, `slime-serialize`, `slime-tasks`, `slime-world`, `bevy_mikktspace`, `blake3`, `block_compression`, `glam`, `gltf`, `png`, `uuid`, `zune-jpeg`, `thiserror` | IDs for source files in metadata files beside them, read and written through `slime-serialize`, their check, and the IDs of the assets inside a file; PNG and JPEG images cooked into textures compressed in BC7 or BC5, every level computed; glTF files imported into meshes (MikkTSpace tangents), materials and scenes; the typed properties of their nodes; scene files checked and cooked into the binary encoding of documents ([ADR-0095](adr/0095-cooked-scene-files.md)), and the interface's style sheets, themes and tables of shortcuts ([ADR-0141](adr/0141-style-sheets-and-themes.md), [ADR-0142](adr/0142-input-and-focus.md)); a directory cooked again only where it changed, with its manifest, once or at each change; files written whole or not at all, as the editor writes its levels |
| [`slime-mcp`](../crates/slime-mcp/README.md) | The engine's MCP server: an AI agent drives a running game through it, as a person would ([ADR-0099](adr/0099-ai-agents-drive-the-engine.md), [ADR-0122](adr/0122-the-development-channel.md)); a program, which no game links | `slime-channel`, `rmcp`, `tokio`, `serde`, `base64`, `png` | One game at a time, started with a token and driven through its development channel; its captures, whole or of one view, as PNG images, its views' settings changed, its reports read and the entity at a pixel picked; the elements of its interface found by role and name or at a pixel, acted on and captured ([ADR-0143](adr/0143-what-agents-see-of-the-interface.md)); assets cooked and tests run; an editor, started as a game is, its level edited through steps, undone and redone, its history and journal read and its level written back ([ADR-0135](adr/0135-the-editors-program.md)), and the game that its Play runs driven beside it ([ADR-0165](adr/0165-editing-while-playing.md)) |
| [`slime-devui`](../crates/slime-devui/README.md) | The dev overlay: panels on the engine's interface, in the editor's look, floating over a program's view, and its world panel and editors of reflected values; tooling ([ADR-0081](adr/0081-stage-5a-the-world.md), [ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)) | `slime-assets`, `slime-color`, `slime-gfx`, `slime-platform`, `slime-reflect`, `slime-render`, `slime-ui` (with its `editor-look` feature), `slime-ui-render`, `slime-world` | Panels opened and closed by chords and commands, built when a frame is extracted and drawn with the view's renderer after the view; a world's entities listed by name, the list building only the rows that show, and their components edited through reflection, each edit given as a command for the caller to send; any reflected value edited with the same editors, as a view's settings ([ADR-0127](adr/0127-the-settings-of-a-view.md)) |
| [`slime-editor`](../crates/slime-editor/README.md) | The editor, a client of the engine's public API: its core, a level opened from its scene file, changed through the world's commands grouped into steps, undone and redone, journaled, and written back with its comments ([ADR-0109](adr/0109-stable-references-and-undo.md), [ADR-0134](adr/0134-the-editors-core.md)); its operations, by name; its window, on the engine's interface ([ADR-0156](adr/0156-the-editors-window-and-its-operations.md)), and its viewport, where the user picks ([ADR-0158](adr/0158-the-viewport.md), [ADR-0159](adr/0159-picking-and-the-selection.md)); its core headless, its program on `slime-app` | `slime-app` (with its `development` feature), `slime-assets`, `slime-channel`, `slime-color`, `slime-cook`, `slime-devui`, `slime-gfx`, `slime-platform`, `slime-reflect`, `slime-render`, `slime-serialize`, `slime-ui` (with its feature `editor-look`), `slime-ui-render`, `slime-world`, `slime-world-render`, `serde_json`, `thiserror`, `tracing` | Steps made whole or not at all from commands given as values or as text; undo and redo within a bound of memory; the selection, by persistent identity, which undo and redo restore; a journal of the steps, as text, from which an editor that crashed is recovered; commands that would change what is not under the level's root refused; the level written back whole or not at all, its comments kept, what the editor created named as the file names it. Its program, `run`, on `slime-app`, in a window of its own or without one, which a person uses and tools drive through the development channel alike, its operations run by name, and which recovers its journal as it starts ([ADR-0135](adr/0135-the-editors-program.md), [ADR-0156](adr/0156-the-editors-window-and-its-operations.md)); a viewport that shows the level lit as the game gives, drawn again only when what it shows changed, flown, orbited and panned through, framing the selection ([ADR-0158](adr/0158-the-viewport.md)), where a click, a marquee and Alt and a click select, what is selected outlined, and the View menu shows the base color, the light alone, the normals, the shadows and the wireframe of the selection ([ADR-0159](adr/0159-picking-and-the-selection.md)); two viewports side by side, and Blender's way of the keys and the mouse's buttons ([ADR-0160](adr/0160-two-viewports-and-blenders-way.md)); handles that move, turn and scale what is selected, snapped if asked, each gesture one step through an open step, and drops of the selection on what lies below ([ADR-0161](adr/0161-the-open-step-and-the-handles.md)); entities created, deleted, duplicated, copied and pasted as commands' text on the system's clipboard, grouped and nudged, each a step ([ADR-0162](adr/0162-creating-copying-and-arranging.md)); prefabs placed from the cooked data's scenes, and the history shown in a panel that goes to a step ([ADR-0163](adr/0163-placing-prefabs-and-the-history-panel.md)); a hierarchy of the level's entities, folded, selected, searched and renamed in place ([ADR-0167](adr/0167-the-hierarchy.md)); Play, the game run on the level as the editor holds it, paused, stepped and stopped from a toolbar, its log in a console ([ADR-0164](adr/0164-play.md)), the steps made while it runs sent to it ([ADR-0165](adr/0165-editing-while-playing.md)), and the game built by cargo as Play starts it and again as its code changes ([ADR-0166](adr/0166-building-the-game-again.md)) |
| [`slime-examples`](../examples/README.md) | Runnable examples; not part of the engine | `slime-app`, `slime-assets`, `slime-color`, `slime-devui`, `slime-editor`, `slime-gfx`, `slime-platform`, `slime-reflect`, `slime-render`, `slime-ui`, `slime-ui-render`, `slime-world`, `slime-world-render`, `png`, `tracing`; for the tests, `slime-cook` and `gltf` | `hello`, `headless`, `window`, `offscreen`, `scene`, which draws a cooked scene, `world`, which spawns a yard of prefabs from a cooked scene file and draws it from a world, through two views whose settings its overlay changes; `editor`, the editor's window on the yard, or drawn into an image ([ADR-0135](adr/0135-the-editors-program.md), [ADR-0156](adr/0156-the-editors-window-and-its-operations.md)); `text`, the interface's text in several writing systems, rasterized into an image by the CPU ([ADR-0136](adr/0136-the-interfaces-text.md)); `interface`, an interface built as a tree of elements, styled by a style sheet and a dark or a light theme cooked with the sample, and drawn through the renderer's canvas, over a window or into an image, from left to right or mirrored, with a list of 100 000 rows, text in eight writing systems and a view's image in a panel, used with the pointer and the keyboard, a field of text and one of numbers in its bar ([ADR-0145](adr/0145-text-edited.md)), and read and used by an agent, which presses a button that changes its world ([ADR-0143](adr/0143-what-agents-see-of-the-interface.md), [ADR-0137](adr/0137-the-canvas.md), [ADR-0139](adr/0139-text-drawn-and-the-display-list.md), [ADR-0140](adr/0140-the-tree-of-elements.md)); `widgets`, a panel of the editor's kind in the editor's look, every widget of a panel in it, over a window or into an image, used by an agent ([ADR-0148](adr/0148-widgets.md)); the reference image tests, and tests that cook glTF files and draw them |
| [`slime-benches`](../benches/README.md) | The engine's benchmarks, a program whose figures [`measurements.md`](measurements.md) keeps; a client of the engine through its public API only, not part of the engine ([ADR-0104](adr/0104-the-build-ci-and-measurements.md)) | `slime-app`, `slime-assets`, `slime-channel`, `slime-color`, `slime-cook`, `slime-devui`, `slime-editor`, `slime-gfx`, `slime-render`, `slime-ui`, `slime-ui-render`, `slime-world`, `slime-world-render` | The scenarios that its README lists, of rendering, of the world, of loading, of text, of the interface's tree, of the dev overlay's world panel, of the editor's hierarchy and of Play's time to control |
| [`hunnawood`](../games/hunnawood/README.md) | Hunna Wood, the game that every stage grows from Stage 4a on; a client of the engine through its public API only, not part of the engine ([ADR-0055](adr/0055-games-in-the-workspace.md), [ADR-0067](adr/0067-hunna-wood-replaces-nightwatch.md)) | `slime-app`, `slime-assets`, `slime-color`, `slime-gfx`, `slime-platform`, `slime-reflect`, `slime-render`, `slime-serialize`, `slime-world`, `slime-world-render`, `tracing`; with its `development` feature, on by default, `slime-devui` and `slime-ui`, and `slime-app`'s `development` feature | A library, its rules as a simulation and what the player sees as a presentation, and its program ([ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md), [ADR-0121](adr/0121-a-game-as-a-simulation-and-a-presentation.md)); a forest zone, a scene file that places the land and six lanterns of one prefab, and its hero, read from cooked data and spawned in the game's world, its colliders, markers and lights' times of day as the game's components, typed properties of their nodes, the camera behind the hero along where the player looks, the hero's movement and its collisions with boxes turned about the vertical and cylinders, a key that opens a door, a campfire that flickers, the zone drawn while it loads, game time held until the zone is drawn, and paused, slowed down and stepped from the dev overlay, which changes the settings of its view too ([ADR-0127](adr/0127-the-settings-of-a-view.md)), the run kept after a lost GPU device, the run carried into the scenes changed on disk, and the shaders loaded again when their files change (`--watch`); the run saved on F5 and on quit, loaded on F9 and resumed on start, a save checked before it is resumed and held by one copy of the game at a time ([ADR-0088](adr/0088-saving-and-resuming-a-run.md), [ADR-0101](adr/0101-saves-and-identities-that-never-mislead.md), [ADR-0125](adr/0125-saves-per-scene.md)); the development channel in its default build ([ADR-0122](adr/0122-the-development-channel.md)) |
| [`hunnawood-editor`](../games/hunnawood-editor/README.md) | *Hunna Wood*'s editor: the engine's editor, which knows the game's types through its library; a client of the engine, which the game does not depend on ([ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md), [ADR-0135](adr/0135-the-editors-program.md)) | `hunnawood`, `slime-editor`; for its test, `slime-assets`, `slime-world` | The editor's program on the game's zone, in its window or without one; the zone written back with its comments |

## Dependency rules

```text
slime-examples ──▶ slime-app ──┬──▶ slime-platform ──▶ winit, arboard, slime-input,
       │                       │                     accesskit_winit (feature accessibility)
       │                       ├──▶ slime-gfx ──────▶ wgpu
       │                       ├──▶ slime-world (the game's world)
       │                       ├──▶ slime-reflect (the settings and reports of a presentation)
       │                       ├──▶ slime-ui (a presentation's interface: given the window's events,
       │                       │    read by tools and screen readers)
       │                       ├──▶ slime-channel, slime-serialize (feature development)
       │                       │    slime-channel ──▶ getrandom, slime-reflect, slime-serialize
       │                       └──▶ accesskit (feature accessibility: the tree of screen readers)
       ├──▶ slime-render ──────────▶ slime-gfx
       ├──▶ slime-assets (cooked data); the tests: slime-cook (they cook first)
       ├──▶ slime-world-render ──▶ slime-world, slime-render (a world drawn)
       ├──▶ slime-editor (the editor's program, on the yard)
       ├──▶ slime-ui (a tree of elements laid out, text laid out and rasterized)
       ├──▶ slime-ui-render (a display list drawn)
       ├──▶ slime-devui ───┬───────▶ slime-ui (feature editor-look), slime-ui-render, slime-render
       │                   │         (the panels, drawn with the view's renderer)
       │                   ├───────▶ slime-gfx, slime-platform (the target, the window's size)
       │                   └───────▶ slime-world, slime-reflect, slime-assets, slime-color (the world panel)
       └──▶ slime-platform, slime-gfx, slime-color, slime-world, slime-reflect (their types: events,
            GPU objects, colors, the world, a component reflected)

hunnawood  ──▶ slime-app, slime-assets, slime-render, slime-platform, slime-gfx, slime-color,
               slime-world, slime-world-render, slime-reflect, slime-serialize (the size of a
               save; its tests, the zones they make), slime-devui and slime-ui (feature
               development) (public API only)

slime-mcp ──▶ slime-channel (a game or an editor driven), rmcp, tokio (the Model Context Protocol)

hunnawood-editor ──▶ hunnawood (the game's types), slime-editor (public API only)

slime-editor ──▶ slime-world (a level, its commands), slime-serialize, slime-reflect (its journal,
                 the comments of its file), slime-cook (its metadata file; a file written whole),
                 slime-app, slime-channel, slime-assets (its program, the requests of its tools,
                 the cooked data of the prefabs placed), slime-ui, slime-ui-render, slime-render,
                 slime-world-render, slime-gfx, slime-color, slime-platform, slime-devui (its
                 window, drawn; its viewport; the world panel)

slime-benches ──▶ slime-app (diagnostics), slime-assets, slime-color, slime-cook, slime-devui,
                  slime-gfx, slime-render, slime-ui, slime-ui-render, slime-world,
                  slime-world-render, slime-channel (an editor's program asked to play),
                  slime-editor (its hierarchy) (public API only)

slime-ui ──▶ parley, fontique (text laid out, fonts found), swash (glyphs rasterized),
             taffy (the tree laid out by flexbox and grid), icu_properties (the writing system of
             a character), slime-color (the colors of the styles and the display list),
             slime-reflect, slime-serialize (the styles described, style sheets and themes read),
             slime-input (the input it takes)

slime-ui-render ──▶ slime-ui, slime-render, slime-gfx (a display list drawn through the canvas),
                    etagere (the glyphs placed in the atlas)

slime-cook ──▶ slime-assets, slime-reflect, slime-serialize (metadata files),
               slime-world (the layout of the scene files that it checks)
slime-render ──▶ slime-assets (cooked data)
slime-world ──▶ slime-assets (cooked scenes), slime-serialize (saves, scene files)
slime-world-render ──▶ slime-world, slime-render, slime-gfx, slime-assets (the IDs of what it holds)
slime-gfx, slime-render, slime-cook, slime-world ──▶ slime-color (the color type)
slime-render, slime-cook ──▶ slime-tasks (the engine's task pool)
slime-assets, slime-color, slime-render, slime-world ──▶ slime-reflect ──▶ glam, slime-reflect-derive
                                                    (reflection)
slime-serialize ──▶ slime-reflect, toml_edit (documents)
```

The graph shows the engine's crates and the third-party crates that shape them; the table above
lists every dependency.

- Dependencies between crates point one way: `slime-app` depends on `slime-platform` and
  `slime-gfx`, never the reverse, and `slime-platform` and `slime-gfx` do not depend on each
  other: the window reaches the GPU layer only as `raw-window-handle` traits. `slime-app` also
  depends on `slime-world`, since it owns the game's world
  ([ADR-0079](adr/0079-the-engines-own-world.md)), and on `slime-ui`, since it connects the
  interface that a presentation gives (`Presentation::ui`) to the window, its events first, the
  clipboard, the pointer's shape, the input method and its time
  ([ADR-0145](adr/0145-text-edited.md), [ADR-0149](adr/0149-the-looks-foundations.md)), to screen
  readers ([ADR-0146](adr/0146-screen-readers.md)) and to the development channel's tools
  ([ADR-0143](adr/0143-what-agents-see-of-the-interface.md)). AccessKit's types stay out of
  `slime-ui`: with their `accessibility` feature, `slime-app` translates the interface's meaning
  into AccessKit's tree, and `slime-platform`, whose API carries that tree and the requests of
  screen readers, holds the adapter ([ADR-0146](adr/0146-screen-readers.md)).
- `slime-render` depends on `slime-gfx` only and does not use wgpu
  ([ADR-0027](adr/0027-the-render-crate.md)). `slime-app` does not depend on it: a game's
  presentation creates its `Renderer` and its views in `start`, from the GPU of its `Context`, and
  calls them in `extract` and `render`; it names the views it drew to the development channel
  (`Presentation::views`), and gives their settings as reflection describes them
  (`Presentation::view_settings`, [ADR-0127](adr/0127-the-settings-of-a-view.md)).
- No crate besides `slime-gfx` reaches wgpu. The `wgpu` feature of `slime-gfx` gives the engine's
  own tests the wgpu device, which `slime-app`'s windowed lifecycle test makes invalid as a driver
  reset does ([ADR-0031](adr/0031-dev-overlay-with-egui.md), whose rule on this feature outlived the
  overlay on egui that it was written for); games must not enable it.
  `slime-devui` is tooling, a client of the engine like the examples: no engine crate depends on
  it, and a game leaves it out of its build for shipping, as *Hunna Wood* does with its
  `development` feature ([ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)).
- A game, such as `hunnawood`, is a client of the engine like the examples: it uses the public API
  of the engine's crates only, and no engine crate depends on it
  ([ADR-0055](adr/0055-games-in-the-workspace.md)). The benchmarks, `slime-benches`, are a client
  in the same way, so that they measure what a game gets
  ([ADR-0104](adr/0104-the-build-ci-and-measurements.md)).
- The editor, `slime-editor`, is a client of the engine as well, through its public API: no engine
  crate depends on it, but the programs that run it, the example `editor` and a game's editor
  package, such as `hunnawood-editor`. An editor package depends on its game, for the game's types,
  and the game never depends on it, so that a game ships without its editor
  ([ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md),
  [ADR-0135](adr/0135-the-editors-program.md)). Its core needs no window or GPU; its program runs on
  `slime-app`, and so builds with them.
- `slime-world` depends on `slime-assets`, for the cooked scenes it spawns, `slime-color`, for the
  colors of its lights, `slime-reflect`, and `slime-serialize`, for its saves and its scene files,
  and on no GPU or window, so that a world runs headless, in tests, servers and tools
  ([ADR-0079](adr/0079-the-engines-own-world.md), [ADR-0087](adr/0087-the-worlds-documents.md)).
- `slime-color` depends on `slime-reflect` only, so that a crate that holds a color does not need
  the GPU layer for it ([ADR-0079](adr/0079-the-engines-own-world.md)).
- `slime-reflect` depends on glam and its derive only, so that every crate that owns a value type
  can depend on it and describe the type where it is defined: `slime-assets`, `slime-color` and
  `slime-world` do ([ADR-0080](adr/0080-reflection-and-serialization.md)). Every component is
  reflected, so that a crate that defines components depends on it
  ([ADR-0085](adr/0085-reflection.md)).
- `slime-serialize` depends on `slime-reflect` and on a TOML parser only, and no crate that only
  describes its types depends on it: the crates that read and write files do, the cooker for its
  metadata files and the scene files that it cooks, the world for its saves and its scene files,
  *Hunna Wood* for the size of the largest save, which it reads its save up to, the development
  channel for its messages, and `slime-app`, with its `development` feature, for the values that the
  channel reads and writes as text
  ([ADR-0086](adr/0086-serialization.md), [ADR-0095](adr/0095-cooked-scene-files.md),
  [ADR-0122](adr/0122-the-development-channel.md)).
- `slime-channel` depends on reflection and serialization, and on no GPU, window or world, so that
  the MCP server and the tests link it without the engine's runtime. `slime-app` links it only with
  its `development` feature, which a game turns on by default and its build for shipping leaves out.
  `slime-mcp` is a program, on which nothing depends: no game links it
  ([ADR-0122](adr/0122-the-development-channel.md)).
- `slime-ui` depends on no engine crate but `slime-color`, `slime-input`, `slime-reflect` and
  `slime-serialize`, and on no GPU or window, so that the interface runs headless, in tests and on
  servers: what it draws leaves it as a display list, which `slime-ui-render` draws through the
  renderer's canvas, and the window's input reaches it in `slime-input`'s types. parley's,
  fontique's, swash's and taffy's types stay inside it
  ([ADR-0132](adr/0132-the-engines-ui-framework.md)).
- `slime-ui-render` depends on the interface and on the renderer, and neither depends on it, as
  `slime-world-render` stands between the world and the renderer.
- `slime-world-render` depends on the world and on the renderer, and neither depends on it: the
  world holds no GPU object, and the renderer draws any list of instances it is given
  ([ADR-0079](adr/0079-the-engines-own-world.md), [ADR-0084](adr/0084-what-the-world-draws.md)).
- `slime-assets` depends on no engine crate but `slime-reflect`, and on no GPU, so that it runs
  headless; the cooker, `slime-cook`, depends on it, and no game links the cooker
  ([ADR-0069](adr/0069-the-asset-pipeline.md)). `slime-render` depends on `slime-assets` to turn
  cooked data into its own GPU objects: each system loads its own kinds of asset.
- Only `slime-platform` uses winit, and winit's types never appear in a public API
  ([ADR-0011](adr/0011-windowing-with-winit.md)). `slime_platform::Window` exposes the
  `raw-window-handle` traits so that graphics libraries can draw into it.
- Only `slime-gfx` uses wgpu, and wgpu's types never appear in a public API
  ([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)), except through its `wgpu` feature, for the
  engine's own tests (above). `slime-gfx` depends on no other engine crate than `slime-color`, so that it works
  without a windowing system.
- A headless run never touches the windowing system or the GPU: it does not create an event loop
  or open a device, so the windowing libraries and the Vulkan loader, which are loaded at run
  time, are not even needed. The checks run the headless sample with no display server to prove
  it.

## Main flows

### Diagnostics setup

An engine program calls `slime_app::init_diagnostics()` first. It installs a global `tracing`
subscriber writing to standard error (unless one already exists), filtered by the `SLIME_LOG`
environment variable, then a panic hook that logs panics before the standard hook prints them,
then logs one line describing the engine and the machine. From then on, every `tracing` event
from the engine, the game or a dependency goes through that subscriber. When the `SLIME_TRACE`
environment variable names a file, the subscriber also writes there every span entered and left,
the phases of each frame among them, in a format that profilers open
([ADR-0104](adr/0104-the-build-ci-and-measurements.md)). In a program built with `slime-app`'s
`development` feature and started with a token, the subscriber also keeps the last lines that the
log writes, which the [development channel](#the-development-channel) gives the tools. Source:
[`crates/slime-app/src/diagnostics.rs`](../crates/slime-app/src/diagnostics.rs); conventions:
[ADR-0008](adr/0008-errors-logging-and-assertions.md).

### Running a game

A game is a *simulation*, its rules, and a *presentation*, what the player sees
([ADR-0107](adr/0107-the-execution-model.md),
[ADR-0121](adr/0121-a-game-as-a-simulation-and-a-presentation.md)):
it implements `slime_app::Simulation` and `slime_app::Presentation`, and calls `slime_app::run`,
which owns the main loop ([ADR-0010](adr/0010-main-loop-ownership.md)). The simulation changes
the world, from the input as data, without a GPU or a window; the presentation reads the world,
never writes it, and asks for changes as commands. A *host* owns the world, real time and game
time, the input and the commands queued (`slime_app::Host`, in
[`crates/slime-app/src/host.rs`](../crates/slime-app/src/host.rs)); `run` is the host of a game's
program, and a test drives a host itself. A *driver* decides when frames run and what time they
represent; every driver runs a frame the same way (`Frames::run_one` in
[`crates/slime-app/src/app.rs`](../crates/slime-app/src/app.rs)):

```text
frame                                     span "frame", trace level
├── channel.answer(host)                  with the `development` feature and a token: the tools'
│                                         requests answered with the host (below, "The
│                                         development channel")
├── host.frame(simulation, delta)         phases 1 to 4 (Host::frame):
│   ├── FrameClock::advance(delta)        real time: the time the frame represents, limited
│   ├── simulation.apply, in order        1. the commands sent since the last frame
│   │                                        (Context::send), through the simulation, to the
│   │                                        world by default; each refusal logged, and handed
│   │                                        back for the frame (Context::refused)
│   ├── GameClock::advance(real delta)    game time: the delta times the speed, or none while
│   │                                        paused; how many fixed steps are due: n
│   ├── simulation.fixed_step             2. n times, span "fixed_step", between
│   │                                        world.begin_fixed_step() and
│   │                                        world.end_fixed_step(): what moves is drawn between
│   │                                        its two transforms (ADR-0084)
│   ├── simulation.update                 3. once, paused too, span "update": what it changes is
│   │                                        drawn at once
│   ├── world.update_transforms()         4. so that extraction reads them at the cost of a
│   │                                        lookup
│   └── input.end_frame()                 the keys pressed and released in the frame forgotten
└── with a window:
    ├── presentation.extract(ctx, &world) 5. reads the world and makes what the frame draws;
    │                                        span "extract"
    ├── surface.acquire(label)            the window's image, its passes named "window, frame
    │                                     <index>", or none: skip drawing, after an empty
    │                                     submission that lets wgpu release the frame's writes;
    │                                     span "acquire"
    ├── presentation.render(ctx, target)  6. draws into the image, without the world; span
    │                                        "render"
    ├── interface::Shown::update          the pointer's shape and the input method, as the
    │                                     presentation's interface asks (ADR-0145); with the
    │                                     `accessibility` feature, what changed in its meaning sent
    │                                     to screen readers (ADR-0146)
    ├── channel.capture(gpu, target,      a tool asked for the image, or for the part of it that
    │       views, presentation.ui())     one of its views draws, or that an element of the
    │                                     interface shows: copied back to the CPU
    └── image.present()                   hands it to the window; span "present"
```

The run stops before the next frame once the game has asked for it (`request_exit`) or failed it
(`exit_with_error`), the window was closed, the process received a termination signal, or
`config.frame_limit` frames have run. The frame in progress always finishes, and both `stop`
methods, the presentation's then the simulation's, are always called after `start` methods that
succeeded; when a `start` returns an error, the run stops at once and `run` returns it as
`RunError::Init`, and an error passed to `exit_with_error` is returned as `RunError::App` once the
`stop` methods have run. The call order is checked by
[`crates/slime-app/tests/lifecycle.rs`](../crates/slime-app/tests/lifecycle.rs) headless, with the
phases, game time and commands driven by a host, and by
[`crates/slime-app/tests/windowed_lifecycle.rs`](../crates/slime-app/tests/windowed_lifecycle.rs)
with a window, which the checks run in a headless Weston compositor (ADR-0024). The profiling
spans of each frame are checked by
[`crates/slime-app/tests/profiling_spans.rs`](../crates/slime-app/tests/profiling_spans.rs)
headless, and by the `spans` scenario of `windowed_lifecycle.rs` with a window.

**Headless driver** (`AppConfig::window` is `None`), in `app.rs`; the presentation is not called:

```text
slime_app::run
├── init_diagnostics()
├── simulation.start(ctx, &mut world)
├── loop: frame with delta = the fixed step (simulated time), back to back, or only when
│         something asks for one when AppConfig::on_demand says, as the editor's program runs
│         (see "Frames on demand")
└── simulation.stop(ctx, &mut world)
```

**Windowed driver** (`AppConfig::window` is `Some`), in
[`crates/slime-app/src/windowed.rs`](../crates/slime-app/src/windowed.rs). winit owns the event
loop; `slime-platform` adapts its callbacks to the `EventHandler` trait, which the windowed driver
implements:

```text
slime_app::run
├── init_diagnostics()
└── slime_platform::run_event_loop(window config, driver)
    ├── winit "resumed"          create the window → driver.window_created
    │                            → Gpu::new_for_window: the GPU and the window's surface;
    │                              ctx gets the window and the GPU; simulation.start(ctx, &mut
    │                              world), then presentation.start(ctx, &world)
    ├── winit "window_event"     Translator::translate (event.rs) → driver.event
    │                            → Resized: surface.resize; with the `accessibility` feature,
    │                              Event::Accessibility goes to screen readers' side alone
    │                              (below); otherwise the event goes first to the interface of
    │                              Presentation::ui (interface::give, with the system's
    │                              clipboard), then, unless it took it,
    │                              presentation.on_event(ctx, &event); the host's input follows
    │                              the event (Input::handle) unless one of them took a key or a
    │                              button put down; CloseRequested → stop
    ├── winit "window_event"     RedrawRequested → driver.redraw_requested → a lost device
    │                            restored (below); the interface's clock advanced by the
    │                            measured delta (Ui::advance, its transitions, ADR-0149);
    │                            frame with the measured delta
    ├── winit "about_to_wait"    → driver.about_to_wait
    │                            → if the next frame is due: window.request_redraw();
    │                              return WaitUntil(next frame) or Exit
    └── winit "exiting"          → driver.exiting → presentation.stop(ctx), then
                                   simulation.stop(ctx, &mut world); the surface, the GPU and the
                                   window are released
```

Frames run on redraw requests ([ADR-0020](adr/0020-rendering-in-the-main-loop.md)). The engine
asks for one every `1 / AppConfig::max_frame_rate` seconds: between them the event loop sleeps
until the next frame is due or an event arrives. A frame that starts a little late does not shift
the schedule, which keeps the average rate; after falling a whole period behind, the schedule
restarts instead of catching up with frames run back to back (`next_due` in `windowed.rs`). The
windowing system also asks for redraws on its own, during a resize for example, and each of them
runs a frame too; such a frame restarts the schedule as well, so the next frame the engine asks for
comes one period after it. On Windows, this is what keeps the window drawn while the user resizes it: the
system then runs its own loop, in which `about_to_wait` is not called.

### Opening the GPU

`slime_gfx::Gpu::new()` creates a wgpu instance limited to the platform's graphics API (Vulkan on
Linux, Direct3D 12 on Windows), asks for the high-performance adapter, logs it, and opens its
device and queue ([`crates/slime-gfx/src/gpu.rs`](../crates/slime-gfx/src/gpu.rs),
[ADR-0016](adr/0016-gpu-layer-with-wgpu.md)). The device has every limit that its adapter offers,
which must reach the engine's floor, WebGPU's default limits, or `GfxError::BelowFloor` names the
one it misses ([ADR-0114](adr/0114-hardware-tiers-and-the-gpus-limits.md)), and those of the
optional features that the engine uses that the adapter offers: timestamp queries and BC texture
compression. Without a GPU, the adapter is a software one:
lavapipe on Linux, WARP on Windows; the checks force lavapipe on machines that have a GPU by
restricting the Vulkan loader to it with `VK_DRIVER_FILES`. Headless runs never open the GPU. Each
device that opens has an identity that no other device of the process has had, which every object
that it creates carries and each use checks: an object used with another `Gpu` panics, where wgpu,
whose instances each number their objects from zero, would act on the other device's object of the
same number ([ADR-0157](adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md)).

`slime_gfx::Gpu::new_for_window(window, width, height, present_mode)` does the same for a window: it
creates the window's surface from the window's `raw-window-handle` traits first, asks only for
adapters that can present to it, and configures it in the first sRGB format the window offers and
the requested present mode, FIFO when the window does not offer Mailbox
([`crates/slime-gfx/src/surface.rs`](../crates/slime-gfx/src/surface.rs)). Its images may be seen
without their sRGB encoding, as the interface's canvas draws them
([ADR-0137](adr/0137-the-canvas.md)): a window whose graphics API does not allow it, Vulkan without
`VK_KHR_swapchain_mutable_format`, is refused with `GfxError::SurfaceWithoutEncoding`. The surface
keeps the window alive, which is why the windowed driver releases it before the event loop closes
its connection to the windowing system.

### Rendering offscreen

Drawing into an image without a window, as the `offscreen` example and the reference image test
do with the triangle of [`examples/common/triangle.rs`](../examples/common/triangle.rs):

```text
gpu = Gpu::new()
├── create_render_target(label, width, height, format)   the image to draw into, 8-bit,
│                                                        Rgba16Float, or Rg32Uint, two integers
│                                                        per texel, which no pipeline blends;
│                                                        later passes can sample the others
├── create_depth_target(label, width, height)            the depth of what is drawn, for passes
│                                                        that test depth; later passes can sample
│                                                        it (a shadow map)
├── create_multisampled_render_target(label, width, height, format)
│                                                        four samples per pixel, for anti-aliasing;
│                                                        passes only draw into it, and resolve it
├── create_multisampled_depth_target(label, width, height)
│                                                        a depth per sample, for the same passes
├── create_shader_module(label, wgsl)                    rejected → GfxError::Shader
├── create_bind_group_layout(label, entries)             what the shaders read: uniform buffers,
│                                                        each of a declared size, storage
│                                                        buffers, textures and
│                                                        render targets, depth targets, samplers,
│                                                        comparison samplers
├── create_render_pipeline(&descriptor)                  rejected → GfxError::Pipeline (as when
│                                                        its shaders read more than a uniform
│                                                        binding's size); declares
│                                                        its vertex layout (or none: vertices from
│                                                        the vertex index), its fragment stage (or
│                                                        none: depth only), its bind group layouts,
│                                                        depth test and bias, culling, whether
│                                                        it draws into multisampled targets, and
│                                                        whether it lays its colors over the
│                                                        target's by their alpha, straight or
│                                                        premultiplied (Blend::Alpha,
│                                                        Blend::Premultiplied)
├── create_vertex_buffer(label, bytes)
├── create_index_buffer(label, indices)
├── create_uniform_buffer(label, bytes)                  written again with write_uniform_buffer
├── create_storage_buffer(label, bytes)                  values that shaders read by index, written
│                                                        by ranges (write_storage_buffer)
├── create_texture(label, width, height, format, pixels) an image that shaders sample, its levels
│                                                        of detail computed from its 8-bit pixels,
│                                                        of four channels or one (R8Unorm); a
│                                                        region written again with write_texture,
│                                                        as the glyphs' atlas is
├── create_texture_with_levels(label, width, height, format, levels)
│                                                        the same with the caller's levels, in any
│                                                        format (Rgba16Float: f32_to_f16)
├── create_cube_texture(label, size, format, levels)     six faces per level, read by direction
├── create_sampler(label, &descriptor)                   how they read it
├── create_comparison_sampler(label, compare, filter)    how they compare a depth with a depth
│                                                        target's (shadow lookups, PCF)
├── create_bind_group(label, &layout, entries)           the resources, bound together
├── uniform_offset_alignment()                           the step of a bind group's dynamic
│                                                        offsets into a uniform buffer
├── max_texture_dimension()                              the largest side of a texture or a
│                                                        target: the adapter's, at least the
│                                                        floor's (ADR-0114, ADR-0120)
├── max_buffer_size()                                    the largest buffer: the adapter's, at
│                                                        least the floor's
├── create_timer(label, passes)                          a GpuTimer for up to `passes` passes of a
│                                                        submission, when can_time() says the
│                                                        adapter has timestamp queries
├── render(&RenderPassDescriptor { target, clear, depth, timer, resolve }, |pass| ...)
│                                                        one command encoder and render pass; the
│                                                        closure sets the pipeline, the bind groups
│                                                        and the buffers, and draws, into part of
│                                                        the target after set_viewport; a
│                                                        multisampled target is resolved into
│                                                        `resolve`; then submitted, timed by the
│                                                        timer when there is one
├── render_depth(&DepthPassDescriptor { depth, timer }, |pass| ...)
│                                                        a pass without a render target: clears the
│                                                        depth target, draws depth-only pipelines
├── submit(label, |commands| ...)                        several passes (commands.render,
│                                                        commands.render_depth) in one command
│                                                        encoder, then one submission; a timer's
│                                                        passes numbered in recording order
├── read_render_target(&target)                          copy into a buffer, wait for the GPU,
│                                                        map it → Image (8-bit RGBA, top row first)
├── read_render_target_later(&target)                    the copy started, without waiting: a
│                                                        Readback, whose try_read gives the texels
│                                                        once later work showed the copy done
└── read_float_render_target(&target)                    an Rgba16Float target → FloatImage (f32)
```

A frame of several passes runs them one after another: each submitted by its `render` or
`render_depth`, or several in one submission (`submit`), which costs the CPU less
([ADR-0041](adr/0041-render-graph.md)). A later pass binds the targets an earlier one drew into
(`BindingResource::RenderTarget`, `BindingResource::DepthTarget`). A pass cannot sample what it
draws into: wgpu refuses it and the engine panics
([ADR-0033](adr/0033-gpu-layer-for-multi-pass-rendering.md)).
[`crates/slime-gfx/tests/passes.rs`](../crates/slime-gfx/tests/passes.rs) shows each case.

Each GPU object is an engine-owned type holding its wgpu object
([ADR-0017](adr/0017-gpu-resource-types.md); bind groups and depth:
[ADR-0026](adr/0026-resource-binding-and-depth.md); float and sampled targets, depth-only passes:
[ADR-0033](adr/0033-gpu-layer-for-multi-pass-rendering.md)). Shader modules and pipelines are
created inside a wgpu error scope, so that a rejection comes back as an error with wgpu's message,
and every other creation inside an out-of-memory scope, so that a buffer, texture or query set that
the memory cannot hold comes back as `GfxError::OutOfMemory` and the caller can react, while memory
that runs out elsewhere loses the device ([ADR-0044](adr/0044-running-out-of-gpu-memory.md)); any
other wgpu error is a bug and panics ([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)). The test
[`examples/tests/offscreen.rs`](../examples/tests/offscreen.rs) compares the image with a reference
PNG within a tolerance ([ADR-0018](adr/0018-reference-image-tests.md)).

### Rendering a scene

`slime-render` draws meshes with the objects above. The application builds a `Scene` for each
frame, a flat list of instances with the lights, the environment, the exposure and the bloom, and
the renderer draws it through views, each a camera's image drawn into a render target, the
window's image or an offscreen one, or into a rectangle of it, several a frame, each with its own
settings ([ADR-0126](adr/0126-views-apart-from-the-renderer.md),
[ADR-0127](adr/0127-the-settings-of-a-view.md)), in passes: the directional light's
shadows, in cascades, the spot lights' shadows, the scene in high dynamic range, the sky behind it,
the resolve of its samples, the bloom's chain, then tone mapping into the target, ordered and
recorded by a render graph for each view
([`crates/slime-render/src/renderer.rs`](../crates/slime-render/src/renderer.rs),
[`crates/slime-render/src/graph.rs`](../crates/slime-render/src/graph.rs), rules:
[ADR-0027](adr/0027-the-render-crate.md), [ADR-0034](adr/0034-hdr-rendering-and-tone-mapping.md),
[ADR-0035](adr/0035-physically-based-materials.md),
[ADR-0037](adr/0037-lights-in-physical-units-and-pre-exposure.md),
[ADR-0038](adr/0038-shadow-mapping.md),
[ADR-0039](adr/0039-image-based-lighting-computed-on-the-cpu.md),
[ADR-0040](adr/0040-bloom.md), [ADR-0041](adr/0041-render-graph.md),
[ADR-0064](adr/0064-cascaded-shadow-maps.md), [ADR-0065](adr/0065-reading-the-cascades.md),
[ADR-0129](adr/0129-drawing-for-the-editor.md)):

```text
renderer = Renderer::new(&gpu, format)                 the output's format; its kinds of
│                                                      materials registered, the physically
│                                                      based one and the unlit one (kind.rs,
│                                                      ADR-0120, ADR-0129); its shaders and
│                                                      pipelines: for each kind, three for the
│                                                      scene (depth test, into Rgba16Float),
│                                                      culling back faces, front faces or none,
│                                                      for targets of four samples and of one,
│                                                      the same again for a kind that blends,
│                                                      which write no depth, and two depth-only
│                                                      ones for the shadow passes for a kind that
│                                                      casts shadows (the second discards what
│                                                      masked materials discard); the sky's and
│                                                      the lines' in the scene, for both numbers
│                                                      of samples; the resolve's, the bloom's two
│                                                      (down and up), the tone-mapping one (each
│                                                      a fullscreen triangle) and the lines'
│                                                      over the image; the unlit kind's six over
│                                                      the image, opaque and blended, one for
│                                                      each culling (ADR-0161); for each kind, three
│                                                      for picking, one for each culling, into
│                                                      an Rg32Uint target; the BRDF
│                                                      table, computed once per process; a black
│                                                      environment; a texel of depth bound in
│                                                      the place of either shadow atlas until
│                                                      it exists, in the group that reads them,
│                                                      bind group 3; the shadow atlases, when a
│                                                      light first casts shadows into one; the
│                                                      canvas's (canvas.wgsl, ADR-0137)
├── create_view(&gpu, label) → View                    what one image keeps from frame to frame:
│   └── view.set_settings(ViewSettings)                its settings (the default until changed:
│                                                      a render scale of 1, four samples, no
│                                                      debugging, every pass), the buffers of its
│                                                      values and instances, its graph, targets
│                                                      and timers, its stats
├── set_shaders(&gpu, &ShaderSources)                  the pipelines made again from new
│                                                      sources, all or none, the canvas's
│                                                      included: on an error, the ones it had
│                                                      stay (ADR-0077)
├── create_mesh(&gpu, label, &MeshData)                after a check, 48-byte vertices with their
│                                                      tangent and the indices written into the
│                                                      first page of the renderer's meshes with
│                                                      room for both (mesh.rs, ADR-0117), a new
│                                                      page when none has; the box around the
│                                                      positions
├── create_material(&gpu, label, &MaterialData)        a 48-byte uniform buffer and five texture
│                                                      and sampler pairs (one white pixel for
│                                                      each slot left empty) in a bind group,
│                                                      after a check of its values; a texture in
│                                                      the wrong format for its slot panics (in
│                                                      a cooked material, an error);
│                                                      Material::set_values checks new values
│                                                      and writes them into the uniform buffer,
│                                                      for the frames submitted after it
├── create_unlit_material(&gpu, label, &UnlitData)    a 32-byte uniform buffer, a texture and its
│                                                      sampler; its color shown as it is, unexposed
└── render(&gpu, &Scene { lights, environment, exposure, bloom, instances },
    │      &mut [ViewDraw { view, camera, target, viewport, drawings }])
    │                                                  → each view's RenderStats in the view, or
    │                                                  the error of a creation (ADR-0044); draws
    │                                                  the first 16 lights of those that
    │                                                  Light::check accepts from every view's
    │                                                  camera, the others left out with a
    │                                                  warning (ADR-0102); every view prepared,
    │                                                  its values written and its targets made,
    │                                                  before any is recorded: an error draws
    │                                                  nothing; for each view, in turn:
    ├── every position moved by −camera.eye, in f64 (ADR-0048): the GPU draws relative to the
    │   view's camera, so that what is near it keeps its precision however far from the origin
    ├── the camera's projection (camera.rs): in perspective, or orthographic, depth reversed and
    │   linear to its far plane; moved and scaled so that the part of its image that it draws fills
    │   the view's rectangle
    ├── each instance's box tested against the camera's view, its four sides, its near plane and an
    │   orthographic camera's far plane: the scene pass draws only those that may be seen (frustum
    │   culling, ADR-0117)
    ├── the four cascades of the first directional light that casts shadows (shadow.rs,
    │   ADR-0050, ADR-0064): what the camera sees of its whole image up to the light's shadow
    │   distance split along its axis into four slices, each twice as deep as the one before; for
    │   each, its tile of the cascades' atlas, an orthographic projection across the light
    │   covering the sphere around its slice, moved to a texel grid fixed in the world, the
    │   instances that cast shadows whose boxes reach across that square, seen by the camera or
    │   not, and along the light the depths their boxes span
    ├── each spot light that casts shadows, in the scene's order: its tile of the atlas, 2048 or
    │   1024 texels wide by how many cast them, its perspective view from the light, from 5 cm
    │   out, its cone and a margin, up to 80° from its axis, and, for the first view that draws
    │   shadows only, which draws the atlas for every view, the instances whose boxes reach into
    │   it (shadow.rs, ADR-0058, ADR-0061); a view without shadows (ViewSettings::shadows) reads
    │   no tile, and draws no cascade
    ├── the buffer of the shadow passes' views ← the cascades' views, then each spot light's,
    │   then the view of the area of the oldest picking asked of the view (View::pick), one slot
    │   each at the device's alignment; bound at group 3 of the shadow and picking pipelines
    ├── the view's frame uniform ← the camera's projection times its rotation, the sky's
    │   directions from clip space (its inverse; an orthographic camera's axis for every pixel),
    │   the exposure, the number of lights, the shadowed light's index and the mode of debugging,
    │   the direction towards an orthographic camera, the sun's disc (direction, radiance), the
    │   lights (64 bytes each, at most 16, a spot light's with the index of its tile), the spot
    │   lights' tiles' views and places in their atlas, and the cascades' views, places and texel
    │   widths; bind group 0, with the
    │   environment, the BRDF table and their sampler, built again when the scene names another
    │   environment (a black one for none)
    ├── each pass's instances ordered by pipeline, then material, then mesh, each as it first
    │   appears among all the instances, the shadow passes' by mesh alone but for masked
    │   materials: a draw binds only what changed since the one before (ADR-0106), and each run
    │   of one pipeline, material and mesh is one draw (instancing, ADR-0117); at most as many
    │   instances as a storage buffer holds the values of, the last left out with a warning
    ├── the instances' storage buffers, bind group 2 ← each instance that a pass draws: its
    │   relative transform, normal matrix and handedness (−1 when it mirrors), in the scene's
    │   order, then those of the client's instances in the scene (Drawings::in_scene) and over
    │   the image (Drawings::over_image) that the camera sees; and for each draw of each pass,
    │   where the values of its instances are;
    │   grown when a frame needs it, shrunk back after a peak
    ├── the client's lines (drawings.rs), those in the scene then those over the image, relative
    │   to the camera, into the view's buffer of segments, made by the first frame that draws one
    │   and sized as the instances' are; for each point, where the view's rectangle lies in the
    │   target it draws into, and its size in the output's pixels
    ├── the size of the view's targets: the rectangle's times the render scale, once that size
    │   has held still for RESIZE_DELAY (ADR-0127); until then, the size they have, their
    │   image stretched over the rectangle
    ├── the view's render graph, built again when its shape changes (the targets' size, its old
    │   targets dropped and the GPU waited for first, so that it never holds both; the
    │   anti-aliasing; a shadow pass, the sky or the bloom starting or stopping, by the scene or
    │   by the view's settings; the rectangle covering its target or not; a client's drawings at
    │   a point starting or stopping): the passes below, each
    │   declaring the targets it draws into and samples, ordered by them; the HDR target
    │   (Rgba16Float) and the depth target, of four samples per pixel with anti-aliasing, the HDR
    │   target their samples are resolved into, and, while the bloom runs, its levels (the
    │   targets divided by 2^i, rounded to the nearest, to a side of 8 texels); the groups that
    │   read them, built from the passes' declarations; a GpuTimer per submission
    ├── the passes, in the graph's order, sixteen to a submission named after the view and its
    │   target, each timed by its submission's GpuTimer when the adapter can:
    │   ├── the shadow pass into the cascades' atlas, created by the first frame that needs it, a
    │   │   depth-only pass skipped when no light or no instance casts shadows, each view drawing
    │   │   its own cascades in turn, before its scene pass reads them: for each cascade,
    │   │   its tile (set_viewport) and its view's slot at group 3; for each group of the instances
    │   │   that cast shadows and whose boxes reach across its square, the shadow pipeline their
    │   │   material asks for (masked or not), the material when it changes and the pipeline
    │   │   reads it, the mesh's page when it changes, one draw_indexed of the group's instances
    │   ├── the spot lights' shadow pass into the atlas, created by the first frame that needs it,
    │   │   drawn by the first view that draws shadows alone, skipped when no spot light casts
    │   │   shadows: for each such light, its tile (set_viewport) and its view's slot; for each
    │   │   instance that casts shadows and whose box reaches into its cone and range, grouped as
    │   │   above (ADR-0058, ADR-0061)
    │   ├── the scene pass into the multisampled HDR target, or without anti-aliasing the HDR
    │   │   target of one sample, cleared to black, the group of both atlases (a texel in the
    │   │   place of each until it exists) bound at group 3, the shadowed
    │   │   light looked up in the finest cascades that hold each fragment (ADR-0065), each spot
    │   │   light that casts shadows in its tile (ADR-0062): for each group of the instances that
    │   │   the camera sees, the pipeline that its material's double-sidedness and its transform's
    │   │   mirroring choose (set when it changes), its material's bind group 1 and its mesh's
    │   │   page when they change, one draw_indexed of the group's instances from the mesh's base
    │   │   vertex; in a mode of debugging, each fragment writes the normal it is shaded with, or
    │   │   the share of the shadowing lights' light that reaches it, as it is (ADR-0127); the
    │   │   blended surfaces are left to the pass after the sky
    │   ├── unless the view leaves the sky out, or shows a mode of debugging: the sky pass into
    │   │   the scene pass's target, keeping it and the depth target: one triangle at depth 0, at
    │   │   infinity, drawn where the depth test of each sample finds no surface, reads the
    │   │   environment along each pixel's direction, adds the sun's disc, times the exposure
    │   ├── when the scene has blended surfaces or the client draws in the scene: the pass after
    │   │   the sky, into the same targets, keeping them: the client's opaque and masked
    │   │   instances drawn as the scene pass draws the scene's, then the blended surfaces, the
    │   │   scene's and the client's, writing no depth, then the client's lines, six vertices each
    │   │   from the vertex's index (lines.wgsl), a band facing the camera, its width in the
    │   │   output's pixels, laid over by its alpha and the share of each pixel it covers, which
    │   │   the pixel measures against the segment's ends on the screen, where nothing nearer hides
    │   │   it, writing no depth (ADR-0129)
    │   ├── when the client outlines instances that the camera sees (Drawings::outline): with
    │   │   anti-aliasing, the scene's depth resolved into a depth target of one sample, each
    │   │   pixel the farthest of its samples (resolve.wgsl's fs_depth); then the outline's mask,
    │   │   an Rgba8Unorm target with a depth target of its own: the outlined instances, the
    │   │   scene's and the client's, each kind's vs_outline and fs_outline, nearest first, each
    │   │   texel marking that it is covered and whether its depth is not farther than the
    │   │   scene's there (ADR-0159)
    │   ├── with anti-aliasing: the resolve pass into the HDR target of one sample, which the
    │   │   passes below read: one triangle reads the samples of each pixel (textureLoad) and
    │   │   averages them, each weighed by 1 / (1 + its brightest channel), so that bright edges
    │   │   are smooth too (ADR-0060)
    │   ├── unless the scene's bloom is 0, or the view leaves it out or shows a mode of
    │   │   debugging: the bloom's passes (bloom.wgsl): each level filtered down from the one above
    │   │   with Jimenez's 13 taps, then each averaged up with the coarser average through a 3 × 3
    │   │   tent, every level with the same weight, every read mirrored at the edges; no pass reads
    │   │   what it writes
    │   ├── the tone-mapping pass into the view's rectangle of the target (set_viewport), which it
    │   │   clears when the rectangle covers the target and keeps otherwise: one triangle over the
    │   │   rectangle reads the HDR target where each pixel's centre falls in it, its position less
    │   │   the rectangle's corner times the targets' texels per pixel, bilinearly (the texel under
    │   │   it at a scale of 1), mixes in the bloom's first average by the scene's strength,
    │   │   multiplies by 64, applies Khronos PBR Neutral, but for a mode of debugging, dithers an
    │   │   8-bit target with a triangular noise of one step hashed from the pixel's position
    │   │   (ADR-0051), and writes it opaque; an sRGB target encodes it
    │   ├── when the client outlines instances: the outline into the view's rectangle of the
    │   │   target (outline.wgsl): each pixel outside the mask, within the outline's width of a
    │   │   texel that it covers, laid over by the outline's color where the scene shows what is
    │   │   outlined nearby, and by the hidden part's otherwise, fading over its last pixel
    │   └── when the client draws over the image: a pass that clears a depth target of the
    │       output's size, the view's own, then a pass into the view's rectangle of the target,
    │       with that depth: the client's unlit instances, nearer than what they drew before
    │       (fs_over_image, in the output's colors), then its lines, as in the scene, but over
    │       everything (ADR-0161)
    ├── when a picking was asked of the view: in a submission of its own, the instances that the
    │   scene pass and the pass after the sky drew, each kind's vs_pick and fs_pick through the
    │   area's view, each instance's number (Instance::pick) into an Rg32Uint target of the
    │   area's size, with a depth target, cleared to 0; then its copy back, which Pick::take gives
    │   once wgpu has seen it done, a frame or more later (ADR-0130)
    └── the view's RenderStats (View::stats): the scene pass's draw calls, instances and
        triangles, the shadow passes' draw calls, the CPU time of its part of this call, and the
        GPU time of each pass, the bloom's summed and the outline's, a few frames old, the latest
        that came back
```

An environment is made apart from any renderer, which any draws with: `Environment::new(&gpu, &Sky)`
draws the sky on the CPU into a 128-texel cube, prefiltered into four levels by frame tasks of the
engine's pool and the calling thread (`environment.rs`), and uploads it in `Rgba16Float`, with its
sun's disc. A scene, the instances, lights and environment of a frame, owns what it draws: its
instances hold clones of their meshes and materials, counted references, so that a scene built on
one thread is drawn on another ([ADR-0111](adr/0111-the-frame-contract.md),
[ADR-0120](adr/0120-a-frame-that-owns-what-it-draws.md)).

The dev overlay ([`crates/slime-devui/src/overlay.rs`](../crates/slime-devui/src/overlay.rs),
[ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)) draws after the view, in the
output: its panels are roots of the presentation's interface, which the run gives the window's
events first, and its display list is drawn through the canvas of the view's renderer, as any
interface's is (below).

The shader ([`shader.wgsl`](../crates/slime-render/src/shader.wgsl)) transforms each vertex by its
instance's matrix then the camera's, and its normal by the instance's normal matrix (the cofactor
matrix of its 3 × 3 part, proportional to the inverse transpose, so that normals stay perpendicular
to surfaces scaled unevenly, and finite for a mesh flattened along one axis); each fragment is
shaded with glTF's metallic-roughness model (ADR-0035): the GGX distribution, Smith's visibility and
Schlick's Fresnel under each of the scene's lights (ADR-0037), whose illuminance at the fragment is
a directional light's illuminance, or a point or spot light's intensity over the square of the
distance (as at 1 cm within 1 cm), brought to zero at its range and, for a spot light, times its
cone's attenuation, as `KHR_lights_punctual` recommends; the shadowed light times the share that its
cascades let through, the finest that holds the fragment first, each fading into the next across a
band at its edge (ADR-0064, ADR-0065): in each, the share of nine comparisons over 5 × 5 texels,
weighed as Castaño proposed, that find the fragment lit, moved off its triangle by 4.5 of the
cascade's texels times the sine of the light's incidence (ADR-0050, ADR-0063); each spot light that
casts shadows times the same share in its tile of the atlas, the fragment moved off its triangle by
the sine of its tilt from the light's axis (ADR-0062); the environment's light (ADR-0039), the
diffuse light read from its roughest level along the normal and the specular light from the level of
the roughness along the reflection, weighted by the split sum's Fresnel from the BRDF table with the
light that rough surfaces scatter more than once, times the occlusion; the emission added; the
normal tilted by the normal texture in the frame of the tangent, which follows the model matrix, and
of its bitangent, rebuilt per fragment; back faces of double-sided materials shaded with the normal
reversed; fragments of masked materials below their cutoff discarded; the sum multiplied by the
exposure and divided by 64, so that half precision holds light up to 64 × 65 504 for the bloom
(ADR-0040), and brighter light written as 65 504. The material's textures are read at the texture
coordinates of the vertex. Its sky stages turn each pixel, at infinity, into a direction, read the
environment's first level along it and add the sun's disc, its edge blended over a pixel. The
bloom's shader ([`bloom.wgsl`](../crates/slime-render/src/bloom.wgsl)) filters that light down and
averages it back up into a blurred copy, which the tone-mapping shader
([`tone_mapping.wgsl`](../crates/slime-render/src/tone_mapping.wgsl)) mixes in before it maps the
light to the display's range. A scene that the asset pipeline cooked reaches the renderer through
`Renderer::load_scene_in_background` or `Renderer::load_scene`
([below](#cooking-and-loading-assets)), which create its meshes, materials and textures and flatten
its node hierarchy into instances and lights; mirrored nodes keep their
front faces through the renderer. Conventions: world space is right-handed
with +Y up, distances are in metres and light in physical units (lux, candelas, and nits for the
light of surfaces and the sky), the camera looks down −Z, front faces wind counter-clockwise (a
mirrored instance keeps its front faces), tangents follow glTF, depth is reversed, from 1 at the
near plane to 0 at infinity (ADR-0047), colors are linear light, drawn in high dynamic range,
exposed in the scene pass (ADR-0037), tone mapped (ADR-0034), then encoded by the output's format.

### Rendering in a window

The same objects draw into a window. Each frame of a windowed run, `Frames::run_one` asks the
surface for the window's image (`Surface::acquire`), lets the presentation draw into its target
(`Presentation::render`, with `Renderer::render` through its views, or `Gpu::render`), then
presents it (`SurfaceFrame::present`).
The surface handles what the graphics API reports about the window, so that the presentation only
sees whether there is an image to draw:

| Reported | Handled by |
|---|---|
| A new size (from `Event::Resized`) | `Surface::resize`; the surface is configured again before the next image |
| A size above the largest image the device accepts (the adapter's, 8192 pixels in a dimension at least) | Limited to it, with a warning; the images are then smaller than the window |
| A size of zero (minimized window) | No image; the frame runs without drawing |
| Outdated configuration | Configured again, then one more try; no image this frame if still outdated |
| Suboptimal image | Drawn and presented, then configured again |
| Image not ready in time (Vulkan), window occluded | No image this frame; on Direct3D 12, wgpu waits up to one second for a late image, then returns it, and the frame is drawn late; on Windows with Vulkan, an image that wgpu handed out but that is still not free after one second loses the device, and the run stops |
| Surface lost, or a configuration the graphics API refuses | Created again from the window; no image this frame. Lost for good (it cannot be created again, or is lost again each time): no image, and the run stops before the next frame, like after a lost device |
| Device lost (see [Losing the GPU](#losing-the-gpu)) | No image; the run stops before the next frame. An image acquired before the loss was found is released instead of presented, so that the surface can be dropped |

How the window shows the images is the *present mode* (`AppConfig::present_mode`). With Mailbox,
the default, presenting never waits, and the frame scheduler alone paces frames, whether the
window is visible or not; windows that do not offer it fall back to FIFO, with which presenting
can block a frame. The measurements behind this choice are in
[ADR-0021](adr/0021-frame-pacing.md).

In GPU capture tools such as RenderDoc, each render pass is named after its target, so the window's
pass of frame 42 appears as `window, frame 42`, and `RenderPass::debug_group` names groups of
commands inside it; the samples' triangle is drawn in a group named `triangle`. On Vulkan, debug
builds pass these names to the driver, and release builds do with `WGPU_DEBUG=1`; Direct3D 12
receives them in every build ([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)).

### Cooking and loading assets

The asset pipeline ([ADR-0069](adr/0069-the-asset-pipeline.md),
[ADR-0071](adr/0071-cooked-textures.md)) runs in two programs: the cooker before the game, the
game after it. Nothing passes between them but files.

```text
slime-cook cook <sources> <output>                     the cooker's program, before the game runs
                                                       (watch: cooks again at each change)
└── slime_cook::cook(sources, output)
    ├── scan(sources, GiveIds)                         every source file and its ID, from the
    │                                                  `.meta` beside it, given if it has none
    ├── for each glTF file: GltfFile::open(path, ..)   parsed, validated, its buffers read from
    │                                                  files inside the sources only
    │   ├── output/<key>.import there already?         reused if each file that it lists is
    │   │                                              whole: read by its kind's reader, of its
    │   │                                              size
    │   └── otherwise import(file ID, image IDs)       meshes (MikkTSpace tangents), materials,
    │                                                  scenes, the textures of embedded images;
    │                                                  the image files asked for in their roles;
    │                                                  nodes' `slime:` properties checked as
    │                                                  values and kept as a binary document
    │                                                  (ADR-0095)
    ├── for each image, in each role asked, or colors:
    │   name = BLAKE3(role, ID, bytes, cooker's version)
    │   ├── output/<name>.ktx2 there already?          reused if it reads as a texture: nothing
    │   │                                              is cooked again
    │   └── otherwise image::decode                    PNG or JPEG into 8-bit texels
    │       └── texture::cook_texture(image, role)     stretched to whole blocks of 4 × 4 texels
    │           │                                      (ADR-0106), every level, averaged for the
    │           │                                      role, then
    │           └── CookedTexture::to_ktx2             BC7 or BC5 blocks, encoded by background
    │                                                  tasks of the engine's pool; written
    │                                                  under a temporary name of the process's,
    │                                                  forced to the disk, then renamed
    ├── for each scene file: slime_world::outline      its layout, without its values' types;
    │   │                                              refused, by line, for an asset that no
    │   │                                              source makes, a prefab that is not a
    │   │                                              scene, scene files that place each other
    │   └── slime_serialize::encode                    its binary encoding, written to
    │                                                  output/<name>.scenefile unless the file
    │                                                  there holds it; it needs the assets of
    │                                                  its table
    ├── for each style sheet, theme and table of       checked, and written in the binary
    │   shortcuts (.style, .theme, .keys)              encoding (ADR-0141, ADR-0142)
    └── Manifest::to_bytes ──▶ output/manifest         each asset's kind, file, size, source;
                                                       written only if it changed

a game that draws a cooked scene itself, as the `scene` example does (a world reads its scenes
through a `SceneLibrary` and loads their meshes through an `AssetCache`: see "The world" and "Scenes
and prefabs", below)
├── slime_assets::CookedAssets::open(output)           reads the manifest, which names its files
│                                                      inside the directory only; each file is
│                                                      checked, its kind and size, before it is
│                                                      read, and each reader checks what it reads
│                                                      before it uses it (ADR-0096)
├── renderer.load_scene_in_background(gpu, &assets, id)
│   │                                                  slime-render: a SceneLoad, at once, for a
│   │                                                  scene, the file's ID for a glTF file's
│   │                                                  default one (load_scene: then waits)
│   └── as tasks of the engine's pool, a few of the process's loads' tasks at once (loads.rs):
│       ├── assets.scene(id)                           its nodes, and the meshes they place
│       ├── a task per mesh                            assets.mesh(id), its ranges taken in the
│       │                                              renderer's pages of meshes (ADR-0117),
│       │                                              its vertices and indices written into
│       │                                              staging buffers
│       ├── assets.material(id), as meshes name them   each read once; then a task per texture:
│       │                                              assets.texture(id), a texture of the GPU
│       │                                              format of its encoding created, its
│       │                                              levels written into staging buffers
│       ├── the renderer's uploader, on its thread     the copies from the staging buffers,
│       │                                              submitted after each frame, at most
│       │                                              the renderer's budget of bytes per
│       │                                              frame while frames are drawn; the
│       │                                              staging buffers kept for the next
│       │                                              uploads (ADR-0107, ADR-0118)
│       └── once the last task and the last copy end  materials and samplers created, a texture
│                                                      of the wrong kind for its slot refused,
│                                                      nodes placed: the LoadedScene, its
│                                                      instances and its lights in world space
├── load.take(), at each frame                         None while it loads, then the whole scene
└── with --watch, once the scene is shown, at each frame:
    ManifestWatch::poll                                a new manifest, read again now and then:
    └── renderer.reload_scene_in_background(..., &loaded)
                                                       as above, but each mesh and texture of
                                                       the scene loaded before whose cooked file
                                                       the manifest still names is kept; the
                                                       game swaps the scenes when it is loaded

(with --watch, at each frame, from the start)
ShaderWatch::poll                                      the renderer's shader files changed:
└── renderer.set_shaders(&gpu, &sources)               its pipelines made again, or none
                                                       (ADR-0077); after a lost device, the new
                                                       renderer is given the files again
```

A load runs in the background (ADR-0073), as background tasks of the engine's pool, without a
thread of its own: a first task reads the scene, others read one mesh or one texture each and write
it into staging buffers, a few tasks of the process's loads at once, and the renderer's uploader
submits their copies within a budget per frame; the last task or copy to end puts the loaded scene
together (ADR-0107, ADR-0118).
A game that hot-reloads (ADR-0076) watches its manifest, since the cooker writes it last, and only
when what it lists changed.

### The world

A game keeps its entities, their components and their hierarchy in a `slime_world::World`
([ADR-0079](adr/0079-the-engines-own-world.md)): components stored by type, queries that read or
change them ([ADR-0082](adr/0082-the-worlds-storage-and-handles.md)), and, for each entity with a
`Transform`, where it is in the world ([ADR-0083](adr/0083-transforms-and-the-hierarchy.md)):

```text
world.spawn_scene(key, &resolved_scene)             a root entity, then an entity per record of
│                                                    the scene, child of its parent's, with its
│                                                    components; the persistent identity key/path
│                                                    (Scenes and prefabs, below)
├── changing a component                            get_mut and query_mut give a guard (Mut), whose
│                                                    first write stamps the value with the world's
│                                                    tick; insert and remove stamp too, and log what
│                                                    left; a Transform changed so, or set_parent,
│                                                    marks the entity's subtree stale; Hidden added
│                                                    or removed, or set_parent, sets the subtree's
│                                                    hidden flag
├── world.changed::<T>(since)                       what changed after a reader's last tick
│                                                    (world.tick()), the blocks of values that did
│                                                    not change skipped; removed, destroyed and
│                                                    structure tell the rest (ADR-0108, ADR-0119)
├── world.global_transform(entity)                  where it is in the world: a lookup if current,
│                                                    else composed from the nearest current ancestor
└── world.update_transforms()                       each stale subtree computed once, from its top
```

`slime-app`'s host owns the game's world, and calls it around each fixed step: what a step moves
keeps its transform from before the step, and is drawn between the two
([ADR-0084](adr/0084-what-the-world-draws.md)). The world holds no GPU object: its components name
meshes by asset ID, and the GPU objects live in a `slime_render::AssetCache`, which loads them by
itself. `slime-world-render` builds each frame from both, and neither `slime-world` nor
`slime-render` knows the other:

```text
frame
├── world.begin_fixed_step()                        slime-app, before each fixed step: the
│                                                    transforms that change keep where they were
├── simulation.fixed_step                           moves entities
├── world.end_fixed_step()                          slime-app: the transforms computed again;
│                                                    what was teleported forgets where it was
├── simulation.update                               what it changes is drawn at once
├── world.update_transforms()                       slime-app: extraction reads lookups
└── presentation.extract, with the world as &World
    └── FrameBuilder::build(world, cache, gpu,       slime-world-render, which takes no renderer:
        │                   GameTime::alpha)         the cache keeps what it loads through
        ├── cache.poll(gpu)                          the batches that finished, each whole, each
        │                                            mesh that failed returned alone; each
        │                                            load given the rank of its most urgent handle
        │                                            (its priority's class, then the order in
        │                                            which loads started: the lower, the sooner),
        │                                            those that nothing holds stopped; past the
        │                                            budgets, releasable pages and textures dropped,
        │                                            least recently held first (ADR-0128)
        ├── each entity with Meshes and a Transform, where the world draws it at the fraction of
        │   a step (world.interpolated_transform), not hidden by itself or an ancestor: an
        │   instance of each mesh that the cache holds, numbered with its entity's handle
        │   for picking (Entity::to_bits, which Entity::from_bits turns back, ADR-0130)
        ├── cache.request(missing, Priority::Now)    the meshes that it holds no handle of: one
        │                                            batch, loaded in the background by the
        │                                            engine's pool (ADR-0073, ADR-0107), its tasks
        │                                            and copies at its class's rank; a handle kept
        │                                            for each, dropped once the world no longer
        │                                            names the mesh (ADR-0113)
        ├── each entity with a Light: the renderer's light at its origin, along its −Z axis, the
        │   sun first, so that it casts the shadows; beyond the lights that the renderer draws,
        │   frame.scene(near) puts the other directional lights next, then the nearest to a point
        │   that the game gives (ADR-0106, ADR-0126)
        ├── the sky's environment, made again when the game's sky or the sun changed: the first
        │   directional light that shows in the sky
        └── Frame { instances, lights, environment, loading, errors }, which owns what it
            draws (ADR-0111), each light with its entity, so that the presentation changes
            what it shows of one without the world (ADR-0121) → frame.scene(near) →
            renderer.render, at presentation.render, on this thread or another
```

`Picker::cast` answers on the CPU the question that picking asks of the GPU: the entity whose mesh
a ray meets first, against the boxes, then the triangles, of the meshes that the world places, read
from the cooked data ([ADR-0130](adr/0130-picking.md)); `Picker::cast_through` gives every entity
that it meets, the nearest first, which the editor goes through
([ADR-0159](adr/0159-picking-and-the-selection.md)).

After a lost device, the game gives the cache's requests back (`AssetCache::into_requests`), which
drops its GPU objects, and makes a new cache with them and the renderer that replaces the lost one
(`AssetCache::with_requests`), and drops the builder's environment: the builder's handles stay
valid, and what they hold loads again; given a cache made without them, the builder asks it again.
With `--watch`, the game gives new cooked data to `AssetCache::reload`, which loads again what
handles hold, keeping what did not change, and takes the rest at once.

### Scenes and prefabs

A scene is spawned under a key as a level, or placed in another scene as a *prefab*: a *placement*,
with *overrides* of its own ([ADR-0089](adr/0089-scenes-and-prefabs.md)). A game reads its scenes
through a `slime_world::SceneLibrary`, by asset ID: `SceneLibrary::of` reads glTF scenes and scene
files from cooked data, each when it is first resolved, and a game or a test can also give it scenes
one by one ([ADR-0095](adr/0095-cooked-scene-files.md)). The library resolves a scene once, with the
world's registry, into a `ResolvedScene` that the world spawns as often as it needs
([ADR-0091](adr/0091-scene-documents-and-placements.md)), and writes back
([ADR-0092](adr/0092-a-world-written-back-as-a-scene.md)):

```text
library.resolve(world.registry(), id)               slime-world
├── a glTF scene: ResolvedScene::from_cooked        a record per node: Transform (its matrix split
│                                                    into a translation, a rotation, a uniform
│                                                    scale and a stretch), Name, Meshes, Light,
│                                                    Extras, and its typed properties, read
│                                                    strictly through the registry; its ID the
│                                                    node's name
└── a scene file, read strictly                     slime-serialize's Document, from its text or
    │                                               its binary encoding, as cooked
    ├── layout                                      [assets], [root], [entities]: IDs, parents,
    │                                               prefabs
    ├── each prefab resolved first, recursively     a cycle refused; nesting and the entities
    │                                               made limited
    ├── records                                     the file's entities, and each placement's
    │                                               prefab's records under its ID
    ├── components                                  read whole, references as paths from the
    │                                               file's root, assets by the table's names
    └── overrides                                   [removed], [added], [changed], applied to the
                                                    values in a save's order; what the prefab no
                                                    longer has set aside, and kept, as are
                                                    references and parents into it

world.spawn_scene(key, &resolved)                   all or nothing: identities key/path, such as
                                                    zone/lamp 3/light; each component made from
                                                    its value; each placement's prefab and
                                                    overrides kept (World::placement); the
                                                    scene's fingerprint for saves

world.spawn_placement(&prefab, parent)              a placement made while the world runs:
                                                    created/N, its entities created/N/ID

world.write_scene(root)                             the subtree back as a scene file (ADR-0092)
├── records                                         the root's components, then the entities of
│                                                   its scene and those created since, which get
│                                                   IDs that they keep, in the order of a walk,
│                                                   and the identities that they take in the file,
│                                                   which saves say (ADR-0124); a driven component
│                                                   as the scene's record holds it
├── placements                                      each against its prefab's records, by
│                                                   identity: what differs, and the overrides
│                                                   that the scene wrote, set aside or not, kept
│                                                   while the world holds their target; a driven
│                                                   component as the prefab and the scene give it
└── Composer, without transient fields or defaults  the same world, the same bytes; what the
                                                    file cannot hold left out, and reported
```

With new cooked data, `library.reload(assets)` gives the IDs of the scenes whose cooked files
changed, forgets what it read of them, and resolves every scene again when next asked, so that a
scene that places a changed prefab follows it; a game then carries its run into the world spawned
anew (`World::carry`, [ADR-0093](adr/0093-saves-and-hot-reload-with-placements.md)).

### Reflection

Every component is reflected ([ADR-0085](adr/0085-reflection.md)): its type describes itself to a
`slime_reflect::Registry`, which each world keeps, and turns into a `slime_reflect::Value`, plain
data, and back. A world reaches any component by its type's name and a property path, without its
Rust type:

```text
World::new()                                        registers the engine's components and the
│                                                    types their values hold
├── world.insert(entity, component)                 the first of its type registers the type
├── world.register::<T>()                           a type found by name before any entity holds one
└── world.set_property(entity, "Light", path, value)
    ├── storage.value(slot)                          the component as a Value (Reflect::to_value)
    ├── registry.set(type, &mut value, path, new)    the path followed through the value and the
    │                                                type's description; the new part's kind,
    │                                                fields, widths and lengths checked, and its
    │                                                numbers against their fields' ranges
    ├── storage.set_value(slot, &value, tick)        the component read back from the value
    │                                                (Reflect::from_value) in place of the old one,
    │                                                stamped with the world's tick
    └── a Transform set this way marks its subtree stale
```

Errors name the component and the path of the part at fault; the component stays as it was.
Serialization writes and reads the same values (below).

The dev overlay's world panel ([`slime_devui::WorldPanel`](../crates/slime-devui/README.md),
[ADR-0081](adr/0081-stage-5a-the-world.md), [ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md))
is the first editor built on it, as elements of the engine's interface. It reads a world and
changes nothing: the presentation takes the interface's actions, gives them to the panel, which
keeps its own and gives the commands that they ask for, which the host applies at the start of the
next frame ([ADR-0121](adr/0121-a-game-as-a-simulation-and-a-presentation.md)); then the panel
builds its elements in a slot of the interface, again only where what it shows changed, its list
building only the rows that show.

```text
extract
├── overlay.set_window(size, scale_factor)           the window that the panels lie over
├── actions = overlay.take_actions()                 the panels' chords taken, which open and
│                                                    close them
├── panel.act(&mut actions)                          the panel's actions: the filter typed, the
│   │                                                entity selected, and the edits, each a
│   │                                                command: the entity's persistent identity,
│   │                                                the component's name, a path and a value
│   └── for each command: ctx.send(command)
└── panel.build(&mut ui, slot, world)                built again where the entity selected, the
    ├── world.entities(), their Name                 world's structure, the filter or a refusal
    └── world.component_value(selected, name)        changed, each component as a Value through
                                                     its type's description in world.registry();
                                                     its rows where the list shows others
render
└── overlay.draw(gpu, view's renderer, target)       the interface laid out and drawn over the view
the next frame's start
└── world.apply(command)                             the host: the entity found by its identity,
                                                     then world.set_property(...); a refusal
                                                     handed back (Context::refused), which the
                                                     panel shows (WorldPanel::refused)
```

Its editors edit a value outside a world too, `slime_devui::ValueEditor`, which sets the parts
changed within their fields' ranges: the `world` example's and *Hunna Wood*'s overlays edit their
views' settings with it ([ADR-0127](adr/0127-the-settings-of-a-view.md)).

### Serialization

`slime-serialize` writes the values of reflected types as TOML documents and reads them back
([ADR-0086](adr/0086-serialization.md)); its module `format` says how a document holds each kind
of value. The type's description in a registry leads both ways, through the values as a document
holds them: tables, arrays, text, numbers and booleans. The same document is written and read in
the engine's binary encoding too, without its types, through the same reader
([ADR-0094](adr/0094-the-binary-encoding-of-documents.md)).

```text
write(registry, ty, value)                a document of one value, through a Composer
├── Composer::put(place, ty, value)
│   └── Writer::value(ty, value)          the value as a document holds it, by its type's
│                                         description; the types past version 1 noted
└── Composer::finish()                    refused if larger or deeper than a reader reads
    └── text::print(root)                 "$versions" first, then each table's values on their
                                          keys' lines, then its larger tables under headers:
                                          the engine's own writer, LF endings

encode(text)                              the document that a text holds, a composer's too, in
                                          the binary encoding: its strings once, then its
                                          values, each a tag and what it holds; without its
                                          types, which a program that knows none, such as a
                                          cooker, can encode

read(registry, ty, text, mode)            through a Document
├── Document::parse(text, mode)
│   ├── text::parse(text)                 toml_edit's parser: the document's values, and where
│   │                                     each value and key lies; size and depth limited
│   │   or, for Document::parse_binary,
│   ├── binary::decode(bytes)             the same values from the bytes; size, depth, counts
│   │                                     and strings checked, never a panic
│   └── "$format", "$versions"            a later version refused
└── Document::root(ty) → Reader::value    at each value of a type of an earlier version, the
    │                                     type's upgrades first (Registry::upgrade), on the
    │                                     document's value; then its description reads it:
    │                                     kinds, fields by name or alias, defaults, ranges
    └── errors placed by line and column  at the key of a field unknown or given twice, at the
                                          value otherwise; by byte in the binary encoding,
                                          found by walking the bytes again
```

`read_as` and `write_as` do the same for a Rust type, which `from_value` then checks. The cooker
reads and writes its metadata files so, as values of `Metadata`.

### Saving and loading a world

A game *settles* its world once it made the start of a run, and again once it prepared a scene that
it spawned since (`World::settle`): what the world holds then is its *base*, which saves compare
it with ([ADR-0125](adr/0125-saves-per-scene.md)). From then on the world keeps a copy of each
value at its first change, the parent before an entity's first move, and the identity of each
entity settled that the run destroys: a save reads them, and costs what changed. Entities are
matched by their persistent identities: a scene's entity by its key and its path through the
placements, `zone/door`, `zone/lamp 3/light`; one created outside a scene by its number,
`created/3`, or by the key that the game gave it, `camera` (`World::spawn_keyed`). What the run
created takes, when its save loads, the numbers that follow the count of the world it is loaded
into ([ADR-0101](adr/0101-saves-and-identities-that-never-mislead.md)); a world whose start created,
without a key, entities that saves hold cannot be saved
([ADR-0124](adr/0124-driven-components-keys-and-created-entities-in-levels.md)). A save is one
document of several values, written through `slime-serialize`'s `Composer` and read through its
`Document`, references to entities written as persistent identities.

A scene unloaded (`World::unload_scene`), or replaced by another under its key
(`World::spawn_scene`), leaves behind what the run changed of it, kept by its key and its asset, its
references as identities, but what its `Persistence` resets; spawning it again under its key and
settling it applies them. An entity adopted (`World::adopt`) leaves its scene for a key of its own,
its former identity kept, so that a level replaces another without taking it along.

```text
world.settle()                                       slime-world: the base; the copies and the
                                                     identities of what was destroyed forgotten,
                                                     but what was adopted out of a scene; a scene
                                                     spawned again takes back what the world kept
                                                     of it when it unloaded it, what referred to a
                                                     record adopted away, or was under it, given to
                                                     the entity adopted

world.save(&about)                                   slime-world
├── about                                            what the game says of the save, which
│                                                    World::about reads alone
├── layout, next_created, [start], [adopted]         the save's layout, 5; the world's count of
│                                                    created entities; the count that the start
│                                                    reached; the former identities of what the
│                                                    run adopted, with their scenes' assets
├── [scenes], [spawned]                              each scene under a key: its asset ID, name
│                                                    and a fingerprint of what it resolves to,
│                                                    prefabs included; each placement of which
│                                                    the save holds an entity, by its root, with
│                                                    the identity that a scene file written back
│                                                    gives it
├── [removed], [parents]                             the entities settled that the run destroyed;
│                                                    the components removed; the parents changed
├── [added], [changed]                               the components added, whole; what changed of
│                                                    the others since their base, as parts
│                                                    (Registry::difference), but of those that
│                                                    their entity marks driven at its base and
│                                                    still
├── [created]                                        what the run created, and adopted, whole:
│                                                    parent, components and, if a scene file
│                                                    written back holds it, the identity that the
│                                                    file gives it
├── [unloaded.<key>.<asset>]                         what the world keeps of each scene unloaded,
│                                                    laid out as the sections above
└── Composer::finish                                 slime-serialize: the same worlds, the same
                                                     bytes; Unsaved entities and transient fields
                                                     left out

start.load(text, &mut scenes, &mut prepare)          a world the game made as at the start of a run,
│                                                    settled first if it never was
├── read_layout                                      first reading: all that can refuse the save,
│                                                    before the world changes: its layout, which
│                                                    must be 5, and sections, what it spawns,
│                                                    adopts and creates, each identity once, the
│                                                    run's numbers following the world's count,
│                                                    what it removes, the parents, what it keeps
│                                                    of scenes unloaded
├── unload, spawn what ran                           the scenes that the run unloaded unloaded;
│                                                    each scene that the world lacks, or holds
│                                                    another in place of, as the library resolves
│                                                    it now, under its key, the world's first
│                                                    destroyed, what is under it and not of it
│                                                    kept; the placements, one that the world has
│                                                    under the identity that a scene file gave it
│                                                    being that one; what it cannot give set aside
├── adopt again                                      what the run adopted and holds, where the
│                                                    world holds its former entity, of the same
│                                                    scene: adopted again, but what the run
│                                                    destroyed of it
├── create                                           the created and adopted entities spawned
│                                                    under their keys or numbers, so that
│                                                    references can name them; one that the world
│                                                    has under the identity that a scene file
│                                                    gave it is that entity
├── read_values                                      second reading: the components' values,
│                                                    references resolved; what the program does
│                                                    not know left out with a warning, what
│                                                    cannot be read set aside
├── prepare, settle                                  the game prepares each scene spawned, and the
│                                                    world is settled
└── apply                                            removed, parents, added, changed
                                                     (Registry::overlay): what the world no longer
                                                     has set aside; the scenes changed since named;
                                                     then what the run kept of a scene that it
                                                     spawned again and had not settled

run.carry(start, &mut scenes, &mut prepare)          hot reload: the run saved, and loaded into the
                                                     world made from the files as they are now
```

The game's simulation saves and loads a run to a file when a tool asks through the development
channel (`Simulation::save` and `Simulation::load`, see "The development channel" below).

### Editing a level

The editor's core ([`slime_editor::Editor`](../crates/slime-editor/README.md),
[ADR-0134](adr/0134-the-editors-core.md)) holds no world: it changes the one that it is given,
which nothing else changes meanwhile.

```text
Editor::open(world, scenes, file, key)
├── slime_cook::metadata::read(<file>.meta)          the level's asset ID
├── scenes.insert_file(id, name, text)               the level's text, its prefabs cooked
└── world.spawn_scene(key, resolved)                 the key that the game spawns it under

editor.apply(world, label, commands, selection)      a step, whole or not at all
└── for each command, in order                       read as the world is when it comes
    ├── command.write(world)                         its text, for the journal: entities by
    │                                                persistent identity
    ├── world.apply(&command) -> undo                the command that undoes it; Delete's,
    │                                                Restore(Deleted), the subtree recorded
    └── undo held as text, or as Deleted             a refusal undoes the commands before it
                                                     and makes no step

editor.undo(world) / editor.redo(world)              the step's commands applied, those that they
                                                     give back kept to go the other way; the
                                                     selection before or after it restored
editor.write(world)
├── world.write_scene(root)                          the level as a scene file
├── slime_serialize::keep_comments(opened, written)  the comments of the file put back
└── slime_cook::write(file, text)                    whole or not at all
```

Before a command of a step is applied, the editor refuses it if it would change what is not under
the level's root. The journal, when the editor keeps one, starts with the level's entry, its key,
its file's whole path and its text, then holds each step as made, undone and done again, each
selection made apart from a step, and each writing of the level; `Editor::recover` opens the level
from the journal's text and plays every entry again, the writings without writing, then checks that
the file is what the journal last wrote. The editor writes its level only while the file is the text
that it read or last wrote, and never over another program's changes.

The editor's program, `slime_editor::run`, is an application of `slime-app` whose simulation holds
the editor's core and whose presentation is its window, or nothing with `--headless`, its frames
run on demand. It opens its level and holds its journal as it starts, the journal of that file in
the user's data directory unless `--journal` names one, recovering it when it holds steps that the
file does not, and stopping when it cannot read it, play it again or write it
([ADR-0135](adr/0135-the-editors-program.md)). The core and the window share a value, on the one
thread that runs both ([ADR-0156](adr/0156-the-editors-window-and-its-operations.md)):

```text
the window (a presentation)                       the core (the simulation)
extract: the interface's actions                  update: drains the requests
├── a menu's item, a shortcut, the palette ──┐    └── handle(request) ◀── Simulation::edit,
│   → Request::Run { operation, arguments }  │        │                   the channel's Edit,
├── the hierarchy's selection, renames →     ├──▶ shared.requests      converted to requests
│   Select, Apply                            │        ├── a step, undo, redo, select, write
├── the world panel's changes → Apply        │        │
└── Context::request_frame ──────────────────┘        ├── file.open: a new world, the level in it
                                                      └── a window's operation → the window's queue
frame: reads shared ◀──────────────────────────── publish: the level, the history, the steps not
├── the menus, the regions, the status bar            written, the selection, the recent levels,
├── the title (Window::set_title)                     the notices
├── the hierarchy, its rows made again as the structure, the folds or the search change
└── the world panel, built from the world
```

The registry of operations (`slime_editor::OPERATIONS`) names every action; the menus are lists of
them, the table of shortcuts, `editor.keys`, binds keys to their names, and the palette lists those
without parameters. A window's operation, the palette, the sheet of shortcuts or the system's
dialog to open a level, reaches the window through the shared value, and is refused without a
window. The dialog fills in the path that an agent gives `file.open` itself
([ADR-0157](adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md)):

```text
file.choose (Ctrl+O, File, the palette, an agent by name)
└── the window: a modal layer over the window, its backdrop dimming it, a field for the path
    └── extract, unless a tool started the program: Window::choose_file(FileChoice) → DialogId
        └── a thread of its own: rfd's dialog (the portal through libdbus, zenity; Windows')
            └── the loop's proxy → Event::FileChosen { dialog, file }
                └── on_event: a file → the layer closed, Request::Run { file.open, [path] }
                              none → closed, or, sooner than a person answers, the path typed
```

The viewport ([ADR-0158](adr/0158-the-viewport.md)) is a surface of the window's interface whose
image is a render target of its own, drawn with the window's renderer through a view of its own and
an asset cache and frame builder of the editor's, from the cooked data that the core opened. *View*
> *Two viewports* shows two, each a pane of the window with its own camera, view and settings,
sharing the level's meshes; the one navigated draws at the full rate, the other resting
([ADR-0160](adr/0160-two-viewports-and-blenders-way.md)). What the buttons do over a viewport is
read from the editor's table of shortcuts (`Ui::button_command`), Blender's way laid over it when
the user chooses it:

```text
the window's events, the surface's actions              the navigation and picking (no GPU)
├── Pressed / Released over the surface ──────────▶  a gesture: fly, orbit, pan; the cursor held
├── RawMotion, keys W A S D Q E while held ───────▶  the camera turned and moved
├── the left button: a click, a marquee, Alt and ─▶  Wanted, which the window asks the viewport
│   a click let go before the mouse moved             about (picking.rs)
└── view.frame_selection (F) ─────────────────────▶  framed on the selection's bounds
extract: a click's pixel, a marquee's area ─────────▶  View::pick, answered a frame or two later
         Alt and a click ───────────────────────────▶  Picker::cast_through, answered at once
         the answers come back (Pick::take) ────────▶  picking::select: the outermost placement,
                                                        a level deeper, what is behind; the
                                                        core's selection, Request::Select
extract: Viewport::prepare                             when the world's tick, the camera, the size,
├── FrameBuilder::build, the grid, the icons            the settings, the selection or the
├── the selection outlined, boxes, a wireframe          wireframe changed, meshes came, a new
└── a scene and drawings made ready                     size held still, or a picking waits
render: Viewport::draw → Renderer::render into its image; Context::request_frame_at the due size
        then the interface, its surface showing the image (CanvasImage::Target), and the marquee
```

What is selected and what lies below it is outlined from the picking numbers of its instances
(`Drawings::outline`); an entity selected that draws nothing shows as the box of a metre that its
transform places, and *View* > *Wireframe of the selection* draws its meshes' edges, read from the
cooked data, over the image ([ADR-0159](adr/0159-picking-and-the-selection.md)).

The handles move, turn and scale what is selected, a gesture made one step through the core's
*open step* ([ADR-0161](adr/0161-the-open-step-and-the-handles.md)):

```text
the window (handles.rs, no GPU)                    the core
Pressed, bound to select, over a handle
├── Gizmo::of: the pivot, the axes, the size       (the outermost of what is selected)
├── gizmo.hit(tool, ray): the part, by the CPU
└── Drag::start ─────────────────────────────────▶ nothing yet
Moved: drag.commands(ray, snap): each entity's
       whole Transform, from where it started ───▶ Request::Gesture → editor.gesture(world,
                                                   commands): the last ones undone, these applied
                                                   live, neither journaled nor in the history;
                                                   every other change refused meanwhile, and a
                                                   tool's Edit requests kept by the channel
                                                   (Simulation::edits_wait)
Released ────────────────────────────────────────▶ Request::EndGesture → editor.end_gesture: one
                                                   step, Move, Turn or Scale, journaled
Escape ──────────────────────────────────────────▶ Request::CancelGesture → editor.cancel_gesture:
                                                   undone
extract: Viewport::prepare, the handles' meshes as unlit instances over the image
         (Drawings::over_image), with a depth of their own
```

End and Shift+End (`edit.drop`, `edit.drop_bottom`) drop the outermost of what is selected on what
lies below its origin or the bottom of its meshes' box (`Picker::bounds`), by the CPU's ray through
every mesh (`Picker::cast_through`), one step made at once, with a window or without.

Copying, pasting and duplicating go through the world's commands as text
([ADR-0162](adr/0162-creating-copying-and-arranging.md)):

```text
edit.copy (Ctrl+C)
└── Editor::copy(world, roots)
    ├── world.creating_commands(roots)       Place, Delete, Remove, Insert for a placement, what
    │                                        differs from its prefab; Create for any other
    │                                        entity; the components that refer within the copy
    │                                        inserted last
    └── command.write(world) each            a step as the journal writes it, `# do`, then each
                                             command after `# command`, the copied identities
        → shared.clipboard, and the window puts it on the system's clipboard
edit.paste (Ctrl+V): the window reads the system's clipboard, or the editor's copy without one
└── Editor::paste(world, label, text)
    ├── Renaming::of_commands(world, key, commands)   a new identity of the level for each entity
    │                                                 that a Create or a Place names
    └── Command::read_renamed(text, world, scenes, renaming) each, as the step is made: entities,
        parents and references renamed, a parent missing the level's root; the copies selected
edit.duplicate (Ctrl+D): Editor::copy, then Editor::paste, without the clipboard
```

Play runs the game on the level as the editor holds it, a thread of its own building it and
driving it, so that a game that builds or hangs never holds the window
([ADR-0164](adr/0164-play.md), [ADR-0166](adr/0166-building-the-game-again.md)):

```text
play.start (F5, the toolbar)
├── Play's directory, new, in the system's temporary files: <the level's asset ID>.scene, the
│   level's text as writing it would give (Editor::text); save.toml
└── Play's threads (play.rs)
    ├── with Program::build: cargo build --message-format=json-render-diagnostics, in the
    │   editor's profile → the program built (serde_json), its sources listed beside it (.d);
    │   a failure ends Play, cargo's errors in the console
    ├── Program::play(&Played { assets, save, resume, headless }) → the game's command,
    │   SLIME_PLAY_SCENES = Play's directory in its environment, its standard error a pipe
    ├── slime_channel::launch_unless(command, Stop asked) → the game, a token in its environment,
    │   its input a pipe
    │   └── the game: SceneLibrary::of(assets) inserts Play's scene files in place of their cooked
    │       ones (slime-world's `development` feature)
    ├── Client: the controls asked (Pause, Resume, Step, Speed, Stop, Apply), game time asked
    │   after each and at each look, the first state whose time it no longer holds giving
    │   the time to control (Event::Controlled), the process watched; Stop's own watch kills the
    │   game ten seconds later if it still runs
    ├── the game's standard error read to its end: its log
    ├── the sources' times, looked at now and then: one changed since the build → cargo builds
    │   again, the game playing on; another program → Request::Save into Play's save, Stop, the
    │   new build started resuming it (Played::resume), or, the run not saved, on the level that
    │   the editor writes again (Control::Written); a failure → cargo's errors in the console
    └── each event → the editor (Waker): Building, Built, BuildFailed, Restarting, Started,
        Controlled, Time, Log, Refused, Ended → the toolbar's state and its mark, the console and the editor's log;
        the player dropped as the game ends, or the editor quits: the game killed if it still
        runs, Play's directory removed
```

While the game runs, what the editor's steps apply reaches it
([ADR-0165](adr/0165-editing-while-playing.md)), and an agent drives either program:

```text
a step made, undone or done again (Editor::apply, undo, redo), a gesture's once it ends
└── the texts of the commands that it applied, recorded (Editor::record_applied, take_applied);
    an undone deletion's restoration, which has no text, left out and told to the user
Session::handle, at the end of each request answered, once the game runs
└── Control::Apply(each step's texts) → Play's thread → a Request::Apply { commands } per step
    └── the game's channel: Command::read(text, world, Simulation::scenes) and
        Simulation::apply, each in its order; the first refused fails the request
        → Event::Refused → a notification and the editor's log
the MCP server's drive_played_game(true)
├── Edit::Played → the editor: the game's address and token, once it runs
└── a Client of the game's own: the game's tools reach the game, the editor's the editor;
    drive_played_game(false), or the game gone, points them back at the editor
```

### Laying out text

The interface's text ([`slime_ui::text`](../crates/slime-ui/README.md),
[ADR-0136](adr/0136-the-interfaces-text.md)) is laid out and rasterized on the CPU:

```text
Fonts::new() / Fonts::with_system_fonts()    the fonts, and the system's for a program that asks
fonts.register(bytes)                        a font file's families, which stacks name
fonts.lay_out(text, &style, scale, bounds)   a TextLayout: shaped, ordered, broken, aligned,
│                                            cut with an ellipsis to the box
├── layout.lines(), layout.runs()            what is drawn, in pixels of the screen
├── layout.cluster_at / position_at / caret / selection   what a point hits, carets, selections
└── run.placed()                             each glyph at a whole pixel, with its GlyphKey
    └── rasterizer.rasterize(&fonts, key)    its image: coverage, or color
TextCache::get(...)                          a text kept from before, its lines broken again for
                                             another box, or laid out and kept within a budget,
                                             shared (Arc<TextLayout>) with the display lists
```

A style names its fonts in a stack, in which a character takes the first font that has it. With
the system's fonts, what no font of the stack has falls back on the system's, and the family found
is remembered for the writing system of the characters that it drew, and named for that writing
system's characters from then on, so that the system is asked once for each writing system and
language. A text cut by its box shows its start, laid out again with an ellipsis. A glyph's key holds
its face, its size and the position between two pixels that its origin falls at, so that equal
keys share one image.

### Building and laying out an interface

An interface is a tree of elements that lasts from one frame to the next
([`slime_ui::tree::Ui`](../crates/slime-ui/README.md),
[ADR-0140](adr/0140-the-tree-of-elements.md)), built by code, laid out by taffy's flexbox and grid
through its traits on the tree's arena:

```text
Ui::new(fonts)                                an interface, its texts laid out with fonts
ui.add_root(key, settings, style)             a root: its key in paths, a size, a scale, a
                                              direction
ui.build(parent, revision, |children| ...)    the builder runs when revision moved, or when a list
│                                             below it shows other rows
├── children.add(key, element, |children| ...)   matched with the element of that key and kind
│                                             before, kept with its state and scroll, or made;
│                                             what changed marks it laid out or drawn again
└── children.add_slot(key, element)           a panel whose children its own builder gives
ui.scroll_to(id, offset)                      what lies below placed again, not laid out
ui.frame() -> (DisplayList, FrameStats)
├── each root marked: styles converted at its scale, taffy run from it, labels measured by the
│   TextCache, boxes rounded to whole pixels; each list's rows laid out alone
├── each element placed on its root, scrolled content moved and cut, a root from right to left
│   mirrored; an element whose box moved is drawn again
├── each element drawn again where it changed: its shadow, box, text or image, kept
└── the display list gathered, each element before its children, with its clip
```

A frame where nothing changed lays out and draws nothing. A list's builder adds the rows that show
(`Children::rows`), from the list's box when last laid out and its scroll; each is laid out alone,
at the list's width and the height of a row, so that a list costs what it shows.

### Styling an interface

Style sheets and themes ([`slime_ui::theme`](../crates/slime-ui/README.md),
[ADR-0141](adr/0141-style-sheets-and-themes.md)) are cooked as scene files are, and read when an
interface takes them; the editor's look, behind `slime-ui`'s `editor-look` feature, is built into
`slime-ui` and read from its text (`slime_ui::look`), as the dev overlay does
([ADR-0149](adr/0149-the-looks-foundations.md)):

```text
interface.style, dark.theme                    sources: rules, and the named values they give
slime-cook cook                                each into its binary encoding, its syntax checked
CookedAssets::style_sheet(id) / theme(id)      their bytes
StyleSheet::from_binary / Theme::from_binary
ui.set_style(&[sheet], &[base, dark])          every rule read as a part of a Style, its "$name"
│                                              given the value of the last of the stacked themes
│                                              that gives it, values that name others or compute
│                                              colors in OKLCH worked out first, and its "#rrggbb"
│                                              a color, through slime-serialize's
│                                              References::value; every element marked to be
│                                              styled again
ui.frame()
└── each marked element: the rules that select it, by kind, classes, key, states and ancestor, in
    their order, then the fields that its own style sets; the style made kept for those rules and
    that own style, shared; a style that changed lays the element out again or draws it again
```

A theme changed while a program runs is cooked again, found by a `ManifestWatch`, and given to
`Ui::set_style` again; a document at fault is refused, and the interface keeps its style.

### Input reaching an interface

A person's input reaches an interface in the engine's vocabulary of input, `slime-input`, which
`slime-platform` translates winit's events into ([ADR-0142](adr/0142-input-and-focus.md)), and
which `slime-app` gives the presentation's interface first ([ADR-0145](adr/0145-text-edited.md)):

```text
Event::CursorMoved / CursorLeft                 the window's events, which slime-app gives the
Event::MouseButton / MouseWheel                 interface of Presentation::ui
Event::ModifiersChanged / Key / Ime
ui.pointer_moved / pointer_left                 the button under the pointer hovered, pressed, held;
ui.pointer_button / wheel                       a field focused, its caret placed, its text selected
ui.modifiers / key / ime                        what the focused field types and edits with, then a
│                                               shortcut's command, or an intent: Tab, the arrows,
│                                               Enter and Space, Escape
├── intents that move the focus                 done by the interface: Tab in the tree's order, the
│                                               arrows to the nearest on screen, within the focus's
│                                               scope, a modal root's or the focus's root
└── a press, accept, cancel, a command,         queued
    a field changed or committed
the call's answer, taken or not                 an event taken reaches neither Presentation::on_event
                                                nor the game's input
ui.act(action)                                  an agent's action, by the same path
ui.take_actions()                               the program reads the queue, and does what it asks
ui.cursor(), ui.text_input()                    slime-app, after each frame drawn: the pointer's
                                                shape, the input method allowed beside the caret
```

The interface takes an event when it reaches it: a press on what it draws, a button, a field, a
panel's box or a label's text, and its release; a key that a focused field takes, that a shortcut
names or that navigates while an element holds a focus that takes the keys, Tab alone giving the
focus where none is held, unless the program kept Tab for itself (`Ui::set_tab_enters`), and F6
moving it from one root to the next; a wheel over a box that scrolls or a list, or over what it
draws; and every press, key and wheel while a modal root is open. A press that reaches no button
and no field takes the focus away, the field that held it committing; one that reaches a button
or a widget gives it a focus that takes no keys, which reach the program, unless it lies in a
menu, a list or a dialog ([ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md),
[ADR-0153](adr/0153-tab-and-f6-in-a-games-interface.md)). A game with no focus that takes the keys
and no modal root open receives every key but those its shortcuts name, Tab unless its interface
lets Tab in, and F6 while a root holds what can take the focus. `slime-app` gives the interface the
system's clipboard, which a field copies to and pastes from.

A popup or a dialog is a root of its own at a place on the window's surface; a modal one keeps the
pointer from the roots below and the focus among its elements, and gives the focus back when it is
removed.

What opens over a panel is a *layer*, a root that the interface places
([ADR-0150](adr/0150-layers.md)): the program opens it beside an element, at a point or floating,
and builds it as any root; a list to choose from and a tooltip open theirs alone. Each frame, the
interface lays out a layer's content, then places it from its anchor's box within the window,
turned or moved where it lacks room, and lays it out again when its anchor moves:

```text
ui.open_layer(key, Layer { place, modal, dismiss })  a root over the others, named by key, its
                                                    scale and direction its anchor's root's
frame: settle_rest                              the pointer resting: a tooltip shown, a submenu
                                                opened by a press, as the program reads it
       settle_layers                            surfaces taken from the anchors' roots; layers
                                                whose anchor went closed
       lay_out(layer) → place_layer             content laid out, then placed beside its anchor
       follow_anchors                           layers whose anchor a layout or a scroll moved,
                                                placed again
press outside, Escape                           dismissable layers closed, those over them too,
                                                the focus given back, ActionKind::Dismissed queued
```

Menus, their bar and lists to choose from move the keys through their items in the interface; a
press on an item reaches the program, which builds what it opens. The interface opens some layers
for itself, which tell no one as they close: tooltips, the palette of commands and the sheet of
shortcuts, which it builds from the commands that the program names and from the tables of
shortcuts, and the stack of notifications, which it keeps from what the program tells it; the
palette gives its command to the program's queue, as a shortcut does. A floating panel moves and
sizes its layer, whose place the program reads to keep it.

### What an agent sees of an interface

A presentation gives its interface to `slime-app` (`Presentation::ui`), which answers the tools of
the development channel and the MCP server from it, at the start of a frame, from the interface as
the last frame left it ([ADR-0143](adr/0143-what-agents-see-of-the-interface.md)):

```text
find_elements / element_at          the MCP server's tools, through the development channel
Request::Elements / ElementAt       slime-app: ui.meaning(), ui.meaning_at(x, y), made when asked;
                                    each element's role, name, value, states, actions, bounds
                                    and path
act_on_interface → Request::Act     slime-app: the element found by its path, the action checked
                                    against what it takes now, then ui.act, ui.set_focus,
                                    ui.scroll_to or ui.set_value, as a person's input
ui.take_actions()                   the program, as that frame draws: a press it turns into a
                                    command of the world, Context::send, applied at the next one
capture_element → CaptureElement    slime-app, once the next frame is drawn: the part of the
                                    window's image where the element shows
```

### What a screen reader reads of an interface

Built with `slime-app`'s `accessibility` feature, a windowed run gives screen readers the tree of
its window through AccessKit's adapter, which `slime-platform` makes before the window shows
([ADR-0146](adr/0146-screen-readers.md)). The interface marks each element whose meaning changes,
and gives what changed in its own types; `slime-app` alone translates them:

```text
a screen reader begins to read          the adapter, from its own thread → Event::Accessibility
                                        (TreeRequested), delivered on the event loop's thread
slime-app                               ui.take_meaning_changes(whole): every element that means
                                        something → AccessKit's update, the window's node at the
                                        root; Window::update_accessibility
each frame drawn, while one reads       ui.take_meaning_changes: the elements marked since, each
                                        with its children, a field with its runs of text; the
                                        window's node when the top or the size changed
the event loop                          gives the adapter the updates once the handler's call returns
a screen reader asks: a click, the      Event::Accessibility(Action) → slime-app: the element found
focus, a value, a selection, a scroll   by its number, then ui.act, ui.set_focus, ui.set_value,
                                        ui.select_text or ui.scroll_to, as a person's input
```

An element is marked as its role, name, value, states, actions, bounds or children change, and the
element above it that means something when it comes, goes, or comes to mean something or nothing;
a modal root opened or closed makes the next changes whole. No screen reader, no update: the
adapter does nothing, and the marks that no one takes are kept within a bound, past which the next
changes are whole.

### Drawing an interface

The renderer draws an interface as a *canvas*
([`slime_render::Canvas`](../crates/slime-render/README.md),
[ADR-0137](adr/0137-the-canvas.md)), after the views, in a pass of its own:

```text
Canvas::new()                                 what an interface draws in a frame
canvas.add_image(CanvasImage::Texture(&t))    an image that items read: a texture, of colors or of
canvas.add_image(CanvasImage::Target(&rt))    coverage, or a target that a view drew
canvas.draw_box / draw_shadow / draw_gradient items in their order, each cut to its clip; a
       / draw_image                           gradient's colors mixed between its corners
renderer.draw_canvas(gpu, &canvas, &target)   one pass over the target, sRGB seen without its
│                                             encoding (RenderTarget::without_srgb)
├── the items written to a storage buffer, their colors mixed as the target mixes them
└── a draw per run of items that read one image: two triangles per item, built from the
    vertex's index, the item drawn by its kind in the fragment shader
```

An item that its clip leaves no pixel of is left out when it is added, and the others' quads are cut
to their clip; the shader computes each pixel's share inside the clip, its shape's coverage, as a
signed distance, and a shadow's, Raph Levien's approximation of a box blurred by a Gaussian. Into an
sRGB target, colors mix as their sRGB values do, as design tools mix them: items' colors are encoded
once each on the CPU, images' texels in the shader.

An interface built by `slime-ui` gives what it draws as a *display list*, data without a GPU, which
`slime-ui-render` draws through the canvas
([ADR-0139](adr/0139-text-drawn-and-the-display-list.md)):

```text
DisplayList::new()                            what an interface draws in a frame, without a GPU
list.push(BoxPrimitive / ShadowPrimitive      boxes, shadows, gradients, texts laid out
          / GradientPrimitive                 (Arc<TextLayout>, shared with the TextCache), and
          / TextPrimitive / ImagePrimitive,   images named by keys, each cut to its clip
          clip)
ui_renderer.draw(gpu, &mut renderer, &fonts,  slime-ui-render's UiRenderer, which gives its
        &list, |key| Option<CanvasImage>,     UiStats:
        &target)
├── the atlas: each glyph that its pages lack rasterized (GlyphRasterizer), placed by etagere in a
│   page of coverage or of colors, and written into it (Gpu::write_texture); past the budget, the
│   glyphs that no draw asked for the longest leave first
├── a canvas: boxes, shadows and gradients as they are, each glyph a part of its page, each image as
│   the program gives it for its key
└── renderer.draw_canvas(gpu, &canvas, &target)
```

### Losing the GPU

The driver can lose the device: it crashes or is reset, the GPU is removed, or it runs out of
memory. Memory that runs out in a creation that no caller hears of gives the device up the same
way
([ADR-0044](adr/0044-running-out-of-gpu-memory.md)). `slime-gfx` records the loss in wgpu's
device-lost callback, and from then on its error handler logs wgpu's errors instead of panicking,
since every use of the device fails, and creations return `GfxError::DeviceLost`, which says why
(`LossKind`); `Gpu::lose` loses a device on purpose, for tests and tools. Before each frame, the
windowed driver checks `Gpu::check()` with the other reasons to stop: after a loss, the run stops
like after an exit request, and `run` returns `RunError::Gfx` with `GfxError::DeviceLost`
([ADR-0022](adr/0022-losing-the-gpu-device.md)). A game that sets
`AppConfig::restore_lost_device` goes on instead
([ADR-0075](adr/0075-recovering-from-a-lost-gpu-device.md)):

```text
between two frames, the device lost
└── WindowedDriver::restore                          slime-app, before deciding to stop
    ├── Presentation::gpu_lost(ctx)                  the presentation drops its objects of the
    │                                                lost device, and its clones of the Gpu
    ├── Context::take_gpu()                          the run's last reference but the surface's
    ├── Surface::restore(lost)                       slime-gfx: the lost device and its surface
    │   │                                            released, the window's surface created
    │   │                                            again through the same wgpu instance
    │   └── Gpu::open(instance, surface, lost)       a new device; refused after a loss to memory,
    │                                                or for a CPU adapter after a hardware one
    ├── Context::set_gpu(new, format)
    └── Presentation::gpu_restored(ctx, &world)      the presentation creates its objects again
```

A game that draws its world drops its renderer and its `AssetCache` in `gpu_lost`, keeping the
cache's requests (`AssetCache::into_requests`), and releases its `FrameBuilder`'s environment; its
world, the host's, is untouched, and the cache that it makes with the new renderer and those
requests in `gpu_restored` loads again what their handles hold, or what the world names, for a game
that asks for it anew ([ADR-0084](adr/0084-what-the-world-draws.md),
[ADR-0120](adr/0120-a-frame-that-owns-what-it-draws.md), [ADR-0128](adr/0128-asset-requests.md)).

Nothing may hold the lost device when the new one opens: Direct3D 12 opens no device on an
adapter while a removed one exists in the process. The run does not restore when it is stopping
anyway or its window's surface was lost for good; after three new devices within five minutes, or
when none opens, it gives up and stops, `run` returning `GfxError::DeviceLost`,
`GfxError::CannotRestore` or the error that opening a device gave. A program without
`slime-app` replaces its device with `Gpu::restore`, or `Surface::restore` for a window.

A loss is found wherever the device is used: by the main thread's frame, by a load's task's
creation, by the uploader's submission, by a wait. wgpu marks the device invalid before
it reports the loss, which records it in the `Gpu` that every clone shares; `slime-gfx`
returns `GfxError::DeviceLost` wherever it finds the loss, recording it if wgpu has not yet,
and the presentation that meets it passes it to `Context::exit_with_error`, or returns it from
`gpu_restored`, which leaves it to the run: the run restores between frames whatever found the loss
([ADR-0103](adr/0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md)).

The window's surface can be lost for good as well, when it cannot be created again after a loss or
is lost again each time it is: `Surface::check` reports it, and the run stops the same way with
`GfxError::SurfaceLost` ([ADR-0025](adr/0025-giving-up-a-lost-surface.md)).

### Termination signals

`run` installs, once per process, a handler for SIGINT (Ctrl+C), SIGTERM and SIGHUP
([`crates/slime-app/src/signal.rs`](../crates/slime-app/src/signal.rs)). The handler runs on a
thread of its own: it sets a flag and hands its log line to another thread, so that a blocked
standard error cannot keep a second signal from being handled; before each frame, the driver
records the flag in the `Context` as an exit request, so the run stops like any other: the frame
in progress finishes and the `stop` methods run. A second signal, half a second or more after the
first, ends the process at once with exit code 130; one that comes sooner (a closing terminal sends
SIGHUP twice) is ignored
([ADR-0012](adr/0012-clean-shutdown-on-termination-signals.md)).

### Frame timing

`slime_app::time` has no clock of its own: the host gives `FrameClock` the duration of each frame
(measured, or simulated in headless runs) and gets a `FrameTime` back, real time; its game clock
scales that delta by game time's speed, or stands still while game time is paused or held, and
gives it to `FixedStep`, which says how many fixed steps the frame must run, those asked for one
by one while game time is paused (`Host::frame`,
[ADR-0107](adr/0107-the-execution-model.md)). Source:
[`crates/slime-app/src/time.rs`](../crates/slime-app/src/time.rs); rules:
[ADR-0009](adr/0009-frame-timing.md).

### Frames on demand

A program that sets `AppConfig::on_demand`, the editor's, runs a frame only when something asks for
one ([ADR-0156](adr/0156-the-editors-window-and-its-operations.md)):

```text
asks for a frame                                  slime-app's loop
├── an event of the window ─────────────────────▶ windowed: about_to_wait → LoopControl::Wait,
├── the channel's listener, a request queued       or WaitUntil(the interface's next change);
│   (Listener's wake) ──▶ Waker::wake ──────────▶  a wake (slime_platform::Waker, a user event of
├── a termination signal, the tool's end ─────────  winit's loop) runs it again
├── another thread: Context::waker ───────────▶ headless: waits on the Waker's condition variable
└── after a frame: Context::request_frame,
    Ui::needs_frame, Ui::next_change, the channel's
    work that waits (a capture, steps, a picking)
```

The frames asked one after another follow the schedule of `max_frame_rate`, as a game's do
([ADR-0021](adr/0021-frame-pacing.md)); a game keeps its scheduled frames.

### The development channel

A tool, the engine's MCP server or a test, drives a running game through its *development
channel* ([ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md),
[ADR-0122](adr/0122-the-development-channel.md)). `slime-channel` holds the protocol and both of
its ends; the game's side is in `slime-app`, behind its `development` feature
([`crates/slime-app/src/channel.rs`](../crates/slime-app/src/channel.rs)):

```text
tool                                             game, built with the `development` feature
slime_channel::launch(command)
├── Token::generate (getrandom)
├── starts the game, the token in its ─────────▶ run → Frames::new → DevChannel::open
│   environment only (SLIME_DEV_TOKEN),          ├── Listener::open: 127.0.0.1, a port that the
│   its standard input a pipe that the tool      │   system chooses; a thread accepts connections
│   holds                                        ├── "slime-dev-channel 127.0.0.1:<port>" on its
└── reads its standard output until ◀─────────── │   standard output
    the announcement                             └── a thread waits for its standard input's end,
                                                     the tool's, which stops the run as a
                                                     termination signal does
Client::connect(address, token)
└── Hello { version, token } ──────────────────▶ a thread for the connection, unless too many
                                                 wait for their hello: refused with the reason
                                                 (another version or token, not a hello, too
                                                 large, too late), or Welcome; a later hello is
                                                 answered as no request
Client::request(request) ──────────────────────▶ the connection's thread queues it for the game
                                                 at the start of the next frame,
                                                 DevChannel::answer(host, simulation,
                                                 presentation):
                                                 ├── the steps asked for that have all run: Done
                                                 ├── the pickings whose answers the presentation
                                                 │   gave: Picked, the entity's identity and name;
                                                 │   one whose PickAnswer it dropped: Failed
                                                 ├── the editor's requests and values set that
                                                 │   waited, in their order, once
                                                 │   Simulation::edits_wait says they no longer
                                                 │   do (ADR-0161)
                                                 ├── each request queued, answered with the host's
                                                 │   world, clocks and input, a save or a load, a
                                                 │   value set and an editor's request by the
                                                 │   simulation (Simulation::apply, edit), those
                                                 │   that change an editor's level or selection
                                                 │   kept while Simulation::edits_wait says, a view's
                                                 │   settings and the reports by the presentation
                                                 │   (presentation.views(), view_settings,
                                                 │   set_view_settings, reports()), checked by
                                                 │   their reflection, the interface's elements by
                                                 │   its interface (presentation.ui(), ADR-0143);
                                                 │   a step once its steps have run, over the next
                                                 │   frames; a picking asked of presentation.pick,
                                                 │   at a pixel of a view or of the whole image,
                                                 │   answered a frame or more later (ADR-0130)
                                                 └── a capture after the frame's render, from the
                                                     window's image, before it is shown, cropped
                                                     to the view that it names, or to where an
                                                     element of the interface shows
reply, and an image's pixels ◀────────────────── written by the connection's thread
```

Messages are values of reflected types in the binary encoding of documents
([ADR-0094](adr/0094-the-binary-encoding-of-documents.md)); the values of a game's components cross
as text in the engine's notation, which the game reads with its own registry, so that a tool needs
no type of the game. The MCP server turns each of its tools into one of these requests, or runs the
cooker or `cargo test`; a game started without a token, or built without the feature, opens no
port. The editor's program listens as a game does, and answers the editor's requests
([ADR-0135](adr/0135-the-editors-program.md)).

### Threads

`run` drives the game from the main thread, and the renderer records and submits its
passes on the thread that calls it. The renderer loads cooked scenes, and the asset cache its
batches of meshes, in the background ([above](#cooking-and-loading-assets), ADR-0073): as
background tasks of the engine's pool (`slime-tasks`, ADR-0107), whose threads, the machine's
logical cores but one, live as long as the process, a few of the loads' tasks at once; they create
GPU objects through clones of the `Gpu`. Each renderer has a thread of its own besides, its
uploader's, which submits the copies of what the loads bring, and waits rather than computes
(ADR-0118). The sky's environment is computed by frame tasks of the same pool, and the
cooker encodes textures on it. A game's development channel has a thread that accepts connections,
and one for each connection, which wait rather than compute ([above](#the-development-channel)).
The handler of termination signals runs on a thread of its own, and logs from another
([below](#termination-signals)); with the `accessibility` feature, AccessKit's adapter asks for the
window's tree from its own thread, the event loop answering it ([above](#what-a-screen-reader-reads-of-an-interface)).
A dialog of the system runs on a thread of its own while it is open, which waits for the user and
gives the answer to the event loop ([ADR-0157](adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md)).
What may cross threads is stated, so that
later stages (a render thread, the job system) can rely on it, and checked for most of them when
the tests of the crates that own these types compile (a `threads` module in their `lib.rs`, a test
of its own for the display list); rustdoc states it for the others:

- **Shared between threads** (`Send` and `Sync`): the `Gpu`, whose clones share its device, its
  `Surface` and every GPU object it creates (targets, textures, samplers, buffers, bind groups and
  their layouts, shader modules, pipelines), its uploaders, images read back, meshes, materials,
  environments, cameras, lights, scenes and instances, the renderer, loaded scenes and loads in
  progress, the asset cache, its requests, handles and reports, statistics, views' settings,
  drawings, pickings and what they found, windows, events, the input, the host, its `Context` and
  `AppConfig`, worlds and entity handles, scene libraries and resolved scenes, the
  frame builder of `slime-world-render`, its frames and its picker, reflected values and registries,
  the development channel's tokens, messages and clients, an interface's display list, fonts and
  text cache, the renderer of display lists (`UiRenderer`), the editor's core (`Editor`), and the
  error types of those crates.
  Scenes, their instances and the frames of `slime-world-render` are also `'static`: they own what
  they draw, so that a frame built on one thread is drawn on another (ADR-0120). wgpu keeps its
  error scopes per thread, so GPU objects can be created from several threads. wgpu fails to
  configure a window's surface when another thread submits work while it waits for the GPU, as the
  uploader's thread does: the `Gpu`'s submissions, from any thread, wait while the surface is
  configured.
- **Moved between threads, used by one at a time** (`Send` only): a `GpuTimer`, which reads its
  times back through a cell, and so the renderer's `View` that holds them; `Renderer::render` takes
  it mutably anyway. A `Readback`, which keeps through a cell what wgpu said of its copy. The
  development channel's `Listener` and the requests it hands over, `Incoming`, which answer through
  a channel of the standard library.
- **Kept on the thread that made them** (neither `Send` nor `Sync`): an interface
  (`slime_ui::tree::Ui`), and so the dev overlay that holds one (`slime_devui::Overlay`), which no
  test checks; a presentation uses them on the main thread, where `run` drives it.

## Platforms

The engine targets Linux (`x86_64-unknown-linux-gnu`) and Windows (`x86_64-pc-windows-msvc`)
([ADR-0015](adr/0015-platforms-and-ci.md)). Platform differences are kept in few places:

- winit hides the windowing systems (Wayland and X11 on Linux, Win32 on Windows).
  `slime-platform` has platform-specific code in three places of `event_loop.rs` and one of
  `event.rs`:
  - `backend_name` names the windowing system in the log.
  - On X11, `resumed` gives winit a maximum window size of 65 535 physical pixels, the protocol's
    limit; beyond it, winit panics.
  - On Windows, `exiting` hides the window, which winit destroys only from inside the event loop.
  - On Windows, `Translator::translate` releases both Shift keys on a key-up of either, since
    Windows sends a single key-up for the two.

  Where winit's events differ between platforms, `slime-platform` makes them agree: `Translator` in
  `event.rs` pairs key and mouse button events across focus changes. One difference shapes the main
  loop: on Windows, the system runs its own loop while the user resizes or moves a window, which is
  why frames run on redraw requests.
- AccessKit's adapter hides the systems' services of screen readers, AT-SPI through D-Bus on
  Linux and UI Automation on Windows; `slime-platform` makes the window hidden, then shows it once
  the adapter is made, which Wayland ignores, showing a window with its first frame
  ([ADR-0146](adr/0146-screen-readers.md)).
- arboard hides the systems' clipboards: Wayland's data-control protocol where the compositor gives
  it, X11's otherwise, through XWayland, and Windows' own; on Linux without either, fields copy and
  paste nothing, which the log says (`crates/slime-platform/src/clipboard.rs`).
- `ctrlc` hides signals (Unix) and console events (Windows); `crates/slime-app/src/signal.rs`
  describes how each platform behaves.
- wgpu hides the graphics APIs; `slime-gfx` only chooses which one to ask for on each platform
  (`DEFAULT_BACKENDS` in `crates/slime-gfx/src/gpu.rs`).
- On Windows, `slime-app` asks the console to interpret the escape sequences that color its log,
  through `nu-ansi-term`, and colors it only when the console accepts (`use_colors` in
  `crates/slime-app/src/diagnostics.rs`): Windows Terminal interprets them already, and the older
  console host once asked.
- Tests that cannot run on a platform are compiled out with `#[cfg]`, and their file says why: the
  signal tests run on Unix only, and the two windowed test programs of `slime-app`
  (`windowed_errors.rs` and `windowed_lifecycle.rs`) on Linux only; having no test harness, they
  keep a `main` that does nothing elsewhere.

`scripts/check.sh` compiles the whole workspace for Windows on Linux, and CI runs the tests and
the headless samples on a Windows runner.

## Where to start reading

1. [`README.md`](../README.md) for what the project is and its state.
2. This file for the crate map.
3. [`PLAN.md`](PLAN.md) for the milestone in progress.
4. The `README.md` of the crate you are about to change.
5. [`examples/`](../examples/README.md) for runnable programs that use the engine.

## `unsafe` code

No crate contains `unsafe` code. [ADR-0005](adr/0005-lints-and-unsafe-policy.md) says when it is
allowed and how it must be written.
