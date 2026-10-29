# slime-benches

The engine's benchmarks: scenarios of the size that the engine aims at, each run many times, whose
figures [`docs/measurements.md`](../docs/measurements.md) keeps from one milestone to the next
([ADR-0104](../docs/adr/0104-the-build-ci-and-measurements.md)). A program of the workspace, not a
library: no crate depends on it, and it depends on the engine's crates as a game does, through
their public API.

```sh
cargo run --release -p slime-benches                   # every benchmark not run only on request
cargo run --release -p slime-benches -- world loading  # those whose names start so
cargo run --release -p slime-benches -- loading.zone   # one run only on request, named whole
cargo run --release -p slime-benches -- play.zone      # Play's time to control, in a window
cargo run --release -p slime-benches -- --list         # every name, and what it measures
VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo run --release -p slime-benches   # on lavapipe
scripts/measure.sh "<title>" >> docs/measurements.md   # with the build's figures, as a milestone does
```

It prints the machine (its logical CPUs, its system, the GPU that the engine opened, the build),
then a Markdown table, one row per figure, which [`scripts/measure.sh`](../scripts/measure.sh)
completes with the build's figures. A debug build says so: its figures do not compare with a
release build's. The examples' sample, which the world's benchmarks place, is cooked into
`target/benches/sample` at its first use in each run.

## The benchmarks

Each times a scenario a number of times, and gives the median and the 90th percentile of each
series, or its median, its slowest or a single time, as each row says, with the number of times.
The program installs the engine's diagnostics: the engine's log, from `info` up, reaches standard
error, and `SLIME_TRACE` writes a trace of a run. An argument selects the benchmarks
whose names start with it, except those run only on request, which an argument selects only when
it is their whole name.

| Name | What it measures | For |
|---|---|---|
| `render.instances.10k` | 10 000 cubes of one mesh and eight materials, in turn, lit by a sun that casts shadows, under a sky with bloom, drawn at the screens of the hardware tiers of [ADR-0114](../docs/adr/0114-hardware-tiers-and-the-gpus-limits.md), 1280 × 800, 1920 × 1080 and 3840 × 2160 ([ADR-0138](../docs/adr/0138-the-high-tier-at-3840-2160.md); 2560 × 1440 before it): at each, the CPU of `Renderer::render`, the frame until the GPU finished it, the scene pass on the GPU, the passes on the GPU summed, which the tier's budget of GPU time is compared with, the draw calls, and the instances that the scene pass drew | B3-F02, B3-F05 |
| `render.unseen.10k` | The same 10 000 cubes, with the camera looking at the sky above them: none is in view, so that the scene pass draws none, and the frame's time is that of the instances culled and of the shadows they cast below | B3-F02, B3-F05 |
| `render.instances.100k` | The same with 100 000 cubes | B3-F02, B3-F05 |
| `render.canvas` | Two screens drawn by the canvas ([ADR-0137](../docs/adr/0137-the-canvas.md)) at the hardware tiers' resolutions, each laid out for 1920 × 1080: an editor's, scaled to the tier's height, its columns spread over its width, a background, 100 panels' shadows, 2 000 boxes as rows of lists, a quarter of them bordered, 20 000 glyphs of an atlas of one channel and 16 images, half the boxes and glyphs clipped; and a game's menu over its image, scaled to fit the screen, its image dimmed, 20 panels with their shadows, some 290 boxes, 3 900 glyphs, a part clipped, and 50 icons of an atlas of colors. At each, the CPU of `Renderer::draw_canvas`, the canvas on the GPU, which ADR-0137 bounds, the game's menu at every tier and the editor's screen at the two larger, and the items and draws | [ADR-0137](../docs/adr/0137-the-canvas.md) |
| `render.shaders` | The renderer's pipelines made again from its shaders unchanged, then after an edit of the scene's shader that the driver's cache cannot hold | C7-08 |
| `world.frame.100k` | `FrameBuilder::build` over 100 000 entities that place one of the sample's meshes each, under 1 000 parents, once the cache holds them, and the frame dropped | B2-F06 |
| `world.transforms.100k` | `World::update_transforms` after the 1 000 parents of 100 000 entities moved | B2-F06 |
| `world.changes.100k` | 100 000 entities that place a light each, of which 1 000 change between two reads: the lights changed through `World::get_mut`, then found, twenty times with the lights spread over the world, every hundredth, by a reader that keeps a copy of every light and compares each with it, and by one that asks the world for what changed since it last read (`World::changed`); then twenty times with the lights side by side, by the second | [ADR-0108](../docs/adr/0108-what-the-world-knows-of-its-changes.md) |
| `world.children` | A parent of 10 000, then 20 000 children emptied, each given no parent: twice the children take four times as long while it is quadratic | G27 |
| `world.placements` | A scene file that places a prefab 2 000, then 8 000 times, each with a change, resolved | G26 |
| `world.save.100k` | A level of 100 000 entities, each with a name and a transform, spawned from a cooked scene and settled, of which a run moves 1 000: saved and loaded again five times, the time of a save, of a load into a level spawned and settled, and the save's size: a save costs what changed | [ADR-0112](../docs/adr/0112-levels-saves-and-worlds-for-open-worlds.md), [ADR-0125](../docs/adr/0125-saves-per-scene.md) |
| `world.undo.1k` | A level of 100 000 entities, each with a name and a transform, and in it a subtree of 1 000, a group of nine groups of 110 entities, each with a name, a transform and a light: deleted by the world's command, then restored by the command that deleting gave back, twenty times; the median and the 90th percentile of the deletion, of the restoration and of both. The editor's step, which copies what the deletion recorded when it undoes, is not measured | [ADR-0109](../docs/adr/0109-stable-references-and-undo.md), [ADR-0134](../docs/adr/0134-the-editors-core.md) |
| `world.deep` | A hierarchy 10 000 deep, each entity with a key and a transform and the child of the one before it: spawned from its root down, each given its parent as it is spawned, once; settled, each moved, then saved five times; then, after a fixed step that moved its root and teleported the root's child, where each entity is drawn halfway to the next step, twenty times. Each walk of every entity's ancestors would cost it the square of its depth | G32, [ADR-0168](../docs/adr/0168-rearranging-the-hierarchy.md) |
| `world.sky` | Frames built around a sun that does not move, the sun set where it is at each fixed step, at the fractions of a step at which a display of 144 Hz shows a game stepping at 60 Hz, after a first frame that makes the sky's environment: the frames that made it again | G13 |
| `ui.text.labels.1k` | 1 000 labels of 12 to 30 letters, names of entities and components, laid out each on a line of its own, with the Noto Sans of slime-ui's tests ([`crates/slime-ui/tests/fonts`](../crates/slime-ui/tests/fonts/README.md)), at 14 pixels | [ADR-0132](../docs/adr/0132-the-engines-ui-framework.md), [ADR-0136](../docs/adr/0136-the-interfaces-text.md) |
| `ui.text.paragraphs` | A paragraph of some 2 000 characters of each of seven writing systems, English, Arabic with Latin words and numbers, Hindi, Thai, Japanese, Chinese and English with emoji, the sentences of slime-ui's tests repeated, laid out 400 pixels wide, each font named in its stack; then the English paragraph's lines broken again in another width, which gathers its glyphs again too | [ADR-0132](../docs/adr/0132-the-engines-ui-framework.md), [ADR-0136](../docs/adr/0136-the-interfaces-text.md) |
| `ui.text.raster` | The glyphs of Latin letters, Chinese characters and emoji rasterized at 14, 32 and 64 pixels, without hinting, by one rasterizer: the median of the time per glyph | [ADR-0132](../docs/adr/0132-the-engines-ui-framework.md), [ADR-0136](../docs/adr/0136-the-interfaces-text.md) |
| `ui.draw.menu` | A game's menu drawn from a display list through `slime-ui-render`, at the hardware tiers' resolutions: the panels, slots, rows and buttons of `render.canvas`'s menu, without its icons, laid out for 1920 × 1080 and scaled to fit, and real texts, laid out with the fonts of slime-ui's tests at the screen's scale, some 2 400 glyphs in all. At each, the first draw, which rasterizes its glyphs into the atlas, the renderer warmed before it; the CPU of `UiRenderer::draw` once they are there; the canvas's GPU time, which ADR-0137 bounds as `render.canvas`'s; the glyphs, the items and the atlas's memory | [ADR-0139](../docs/adr/0139-text-drawn-and-the-display-list.md) |
| `ui.tree.editor` | A tree of the editor's size, some 12 500 elements, in a root of 1920 × 1080 at scale 1: a toolbar of 30 buttons, and a hierarchy, an inspector and a browser of assets, each a panel that scrolls, built in a slot by its own revision, with a row, a field of a component of ten and a row for each of 1 500 labels of `ui.text.labels.1k`'s kind, laid out with the Noto Sans of slime-ui's tests, styled by a style sheet of four rules and a dark or a light theme. Its first frame, built, styled, its texts laid out, laid out and drawn into a display list, a new tree each time; a frame after one value of the inspector changed, the inspector's builder run again; a frame with nothing changed; a frame after the theme changed, every element styled again | [ADR-0132](../docs/adr/0132-the-engines-ui-framework.md), [ADR-0140](../docs/adr/0140-the-tree-of-elements.md), [ADR-0141](../docs/adr/0141-style-sheets-and-themes.md) |
| `ui.tree.list.100k` | A list of 100 000 rows of 20 logical pixels in the same root, each row a name and a number, of which only those that show are built: frames, each built and drawn into a display list, while it scrolls by 7 pixels a frame, which shows other rows every third frame, then while it stands still | [ADR-0140](../docs/adr/0140-the-tree-of-elements.md) |
| `devui.world_panel.100k` | The dev overlay's world panel on the engine's interface, `slime_devui::WorldPanel`, in the editor's look, in a root of 380 × 1000 at scale 1, over a world of 100 000 named entities, the one selected holding a transform and a light: a frame standing still, the actions read, the panel built and the interface drawn into a display list; a frame after the entity selected moved, which builds its details again; a frame after the list scrolled by a row; a frame after an entity was renamed, which reads again the names that changed and filters the list again | G46, [ADR-0152](../docs/adr/0152-the-dev-overlay-on-the-engines-interface.md), [ADR-0167](../docs/adr/0167-the-hierarchy.md) |
| `editor.hierarchy.100k` | The editor's hierarchy, `slime_editor::Hierarchy`, in the editor's look, in a root of 340 × 1000 at scale 1, over 100 000 named entities, 1 000 roots of 99 children each, every root unfolded: a frame standing still, the hierarchy built and the interface drawn into a display list; a frame after the list scrolled by a row; a frame after an entity was renamed, which builds the rows in sight again; a frame that runs a search once the typing paused, every name read (`Hierarchy::set_search`); a frame after an entity was spawned, which makes the rows of the tree again | [ADR-0167](../docs/adr/0167-the-hierarchy.md) |
| `loading.batch` | A load of meshes started, on the calling thread, with a manifest of 10 000, then 100 000 assets | G25, B4-F03 |
| `loading.textures` | 16 squares, each drawn with a material of its own whose base color is a texture of 4096 × 4096 texels in BC7, every level of detail with it, 341 MiB in all, written once into `target/benches/textures` without the cooker, loaded and drawn as `loading.zone` is: the case of the audit's B4-F01 | B4-F01 |
| `loading.zone` | The meshes of *Hunna Wood*'s zone, cooked in `target/cooked/hunnawood`, each placed once on a grid and drawn by day, under a sun that casts shadows, at 1280 × 720, while they load with their materials and textures, then once loaded, each frame waited for on the GPU, after frames that made the renderer's targets, its shadows' atlas and the sky, so that a load is not mixed with what a first frame costs: the time from the opening of the cooked zone until a frame draws them whole, the render CPU during the load and after, and the longest frame during the load, built, drawn and waited for. The zone's bytes, not its scene, whose components only the game registers. On request only, since it needs the zone fetched and cooked, as [*Hunna Wood*'s README](../games/hunnawood/README.md) says | B4-F01, B1-F06 |
| `play.zone` | Play's time to control on *Hunna Wood*'s zone: `hunnawood-editor`, built beside this program, started without its window on a copy of the zone, with a journal, data and state of the run's own, plays it five times after a first, which builds the game and fills the system's caches; for each, the time from the request to play to the first state of the game whose time it no longer holds, which Play measures and the editor's log says, and its parts that the editor took writing the level and cargo finding nothing to build. *Hunna Wood* runs in a window only: run it where a compositor draws with the GPU, as a desktop's, not in a headless Weston, where the machine's Vulkan driver refuses the window's surface and the game draws with llvmpipe. The editor's log is kept in `target/benches/play-zone/editor.log`. On request only, since it needs the zone cooked, its editor built and a compositor | [ADR-0110](../docs/adr/0110-how-the-editor-runs-a-games-code.md), [ADR-0166](../docs/adr/0166-building-the-game-again.md) |
| `play.level.100k` | The same with the `editor` example on the examples' yard with 100 000 entities more, a name for each of their 1 000 parents and a parent for each of the others, none drawn, written into `target/benches/play-level`, which the `world` example plays without a window: entities with a transform each would not fit one scene file (`slime_serialize::MAX_VALUES`). On request only, since it needs the example built | [ADR-0110](../docs/adr/0110-how-the-editor-runs-a-games-code.md), [ADR-0166](../docs/adr/0166-building-the-game-again.md) |

The last column names the findings of the audit of 0.4, described in `docs/PLAN.md` until 0.5
(`git show v0.5.0:docs/PLAN.md`), or the decision, whose changes the benchmark measures before and
after.

## Reading the figures

- On lavapipe, which renders on the CPU, `Renderer::render`'s CPU time holds the GPU work of the
  frame's earlier submissions, which lavapipe runs one after the other
  ([`RenderStats::cpu_time`](../crates/slime-render/src/renderer.rs)): it is not a CPU time
  there. `world.transforms.100k`, `world.changes.100k`, `world.children`, `world.placements`,
  `world.save.100k`, `world.undo.1k`, `world.deep`, the `ui.text` and `ui.tree` benchmarks,
  `devui.world_panel.100k` and `loading.batch` do not touch the GPU in what they time; the others
  draw, or make objects on the GPU.
- The `ui.text` benchmarks measure what the study of the UI framework measured
  ([ADR-0132](../docs/adr/0132-the-engines-ui-framework.md)), with fonts named rather than found by
  the system's fallback, as the engine lays text out: a figure within 20 % of the study's, or
  under it, is as the study found.
- The `play` benchmarks start the editor's program of the build that they run in, beside them, which
  plays the game built in the same profile: `cargo run -p slime-benches -- play.zone` measures a
  debug build, the one that a developer plays most. Their medians compare with the time within which
  [ADR-0110](../docs/adr/0110-how-the-editor-runs-a-games-code.md) asks that Play give control on
  the game's largest level.
- `ui.tree.list.100k`'s 90th percentile compares with the budget of a list's frame that
  [ADR-0140](../docs/adr/0140-the-tree-of-elements.md) sets on the development machine, an AMD
  Ryzen 7 5800X: scrolled by 7 pixels a frame over rows of 20, one frame in three builds rows,
  and the median measures those that only move them.
- `devui.world_panel.100k`'s 90th percentiles compare with the same budget, standing still,
  scrolled, or the entity selected changed: there, the panel's cost does not grow with the world's
  entities (G46). A frame whose world's structure changed, an entity spawned or despawned, makes the
  list of entities again; one after a rename reads again the names that changed; both are measured
  apart. `editor.hierarchy.100k`'s compare with the same budget, standing still, scrolled and after
  a rename; a frame after the structure changed makes the rows of the whole tree again, and one
  that runs a search reads every name, both measured apart.
- A frame "until the GPU finished it" includes the wait of `Gpu::wait_idle`, which wakes up to 1 ms
  after the GPU finished: on a GPU whose frames take a few milliseconds, the times of the passes
  on the GPU are the figures to compare. Each frame reports the latest time of each pass that came
  back from the GPU, a few frames old. Their sum leaves out the gaps between the frame's
  submissions, in which the GPU waits: it is the GPU's work for the frame, not a frame time
  ([`GpuTimer`](../crates/slime-gfx/src/timer.rs)).
- `render.canvas`'s and `ui.draw.menu`'s GPU times compare with the interface's budget on each tier,
  scaled to the RX 6900 XT by ADR-0114's ratios, which [ADR-0137](../docs/adr/0137-the-canvas.md)
  sets. Their target is 8-bit sRGB, as a window's image is, so that the colors mix as sRGB values.
- A GPU that also draws a desktop gives figures that move with what the desktop draws: a change is
  compared with runs made just before it, in turn.
- The figures of lavapipe are not those of a GPU: the gains of the renderer are measured on a GPU,
  in a section of its own.
- A figure compares with a figure of the same machine, driver and build only.
- The benchmarks measure; they check nothing, and fail only when a scenario cannot run.

## What it must not do

- Reach wgpu, or any API that a game cannot reach: it measures what a game gets.
- Run in `scripts/check.sh` or CI, whose shared machines give figures that vary from one run to
  the next; CI builds it with the workspace, so that it keeps compiling.
