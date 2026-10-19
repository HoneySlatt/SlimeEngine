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
cargo run --release -p slime-benches -- --list         # every name, and what it measures
VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo run --release -p slime-benches   # on lavapipe
scripts/measure.sh "<title>" >> docs/measurements.md   # with the build's figures, as a milestone does
```

It prints the machine (its logical CPUs, its system, the GPU that the engine opened, the build),
then a Markdown table, one row per figure, which [`scripts/measure.sh`](../scripts/measure.sh)
completes with the build's figures. A debug build says so: its figures do not compare with a
release build's. The examples' sample, which the world's benchmarks place, is cooked into
`target/benches/sample` by the first run.

## The benchmarks

Each times a scenario a number of times, and gives the median and the 90th percentile, or the
median alone when it runs few times; the table says how many. An argument selects the benchmarks
whose names start with it, except those run only on request, which an argument selects only when
it is their whole name.

| Name | What it measures | For |
|---|---|---|
| `render.instances.10k` | 10 000 cubes, each its own draw, of eight materials, lit by a sun that casts shadows, under a sky with bloom, drawn at 1280 × 720: the CPU of `Renderer::render`, the frame until the GPU finished it, the scene pass on the GPU, the draw calls | B3-F02, B3-F05 |
| `render.instances.100k` | The same with 100 000 cubes; on request only, since lavapipe (Mesa 26.2.4) crashes executing such a frame | B3-F02, B3-F05 |
| `render.shaders` | The renderer's pipelines made again from its shaders unchanged, then after an edit of the scene's shader that the driver's cache cannot hold | C7-08 |
| `world.frame.100k` | `FrameBuilder::build` over 100 000 entities that place one of the sample's meshes each, under 1 000 parents, once the cache holds them | B2-F06 |
| `world.transforms.100k` | `World::update_transforms` after the 1 000 parents of 100 000 entities moved | B2-F06 |
| `world.children` | A parent of 10 000, then 20 000 children emptied, each given no parent: twice the children take four times as long while it is quadratic | G27 |
| `world.placements` | A scene file that places a prefab 2 000, then 8 000 times, each with a change, resolved | G26 |
| `world.sky` | Frames built around a sun that does not move, the sun set where it is at each fixed step, at the fractions of a step at which a display of 144 Hz shows a game stepping at 60 Hz: the frames that made the sky's environment again | G13 |
| `loading.batch` | A load of meshes started, on the calling thread, with a manifest of 10 000, then 100 000 assets | G25, B4-F03 |
| `loading.zone` | The meshes of *Hunna Wood*'s zone, cooked in `target/cooked/hunnawood`, each placed once on a grid and drawn by day while they load with their materials and textures, then once loaded: the time until a frame draws them whole, the render CPU during the load and after. The zone's bytes, not its scene, whose components only the game registers. On request only, since it needs the zone fetched and cooked, as [*Hunna Wood*'s README](../games/hunnawood/README.md) says | B4-F01, B1-F06 |

The last column names the findings of [`docs/PLAN.md`](../docs/PLAN.md) whose changes the
benchmark measures before and after.

## Reading the figures

- On lavapipe, which renders on the CPU, `Renderer::render`'s CPU time holds the GPU work of the
  frame's earlier submissions, which lavapipe runs one after the other
  ([`RenderStats::cpu_time`](../crates/slime-render/src/renderer.rs)): it is not a CPU time
  there. The world's and loading's benchmarks do not depend on the GPU.
- A figure compares with a figure of the same machine, driver and build only.
- The benchmarks measure; they check nothing, and fail only when a scenario cannot run.

## What it must not do

- Reach wgpu, or any API that a game cannot reach: it measures what a game gets.
- Run in `scripts/check.sh` or CI, whose shared machines give figures that vary from one run to
  the next; CI builds it with the workspace, so that it keeps compiling.
