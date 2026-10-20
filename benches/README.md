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

Each times a scenario a number of times, and gives the median and the 90th percentile of each
series, or its median, its slowest or a single time, as each row says, with the number of times.
The program installs the engine's diagnostics: the engine's warnings and errors reach standard
error, and `SLIME_TRACE` writes a trace of a run. An argument selects the benchmarks
whose names start with it, except those run only on request, which an argument selects only when
it is their whole name.

| Name | What it measures | For |
|---|---|---|
| `render.instances.10k` | 10 000 cubes, each its own draw, of eight materials, lit by a sun that casts shadows, under a sky with bloom, drawn at the screens of the hardware tiers of [ADR-0114](../docs/adr/0114-hardware-tiers-and-the-gpus-limits.md), 1280 × 800, 1920 × 1080 and 2560 × 1440: at each, the CPU of `Renderer::render`, the frame until the GPU finished it, the scene pass on the GPU, the passes on the GPU summed, which the tier's budget of GPU time is compared with, and the draw calls | B3-F02, B3-F05 |
| `render.unseen.10k` | The same 10 000 cubes, of eight materials in turn, with the camera looking at the sky above them: none is in view, and the frame's time is mostly that of its commands, encoded, submitted and set up by the driver | B3-F02, B3-F05 |
| `render.instances.100k` | The same with 100 000 cubes; on request only, since lavapipe (Mesa 26.2.4) crashes executing such a frame | B3-F02, B3-F05 |
| `render.shaders` | The renderer's pipelines made again from its shaders unchanged, then after an edit of the scene's shader that the driver's cache cannot hold | C7-08 |
| `world.frame.100k` | `FrameBuilder::build` over 100 000 entities that place one of the sample's meshes each, under 1 000 parents, once the cache holds them | B2-F06 |
| `world.transforms.100k` | `World::update_transforms` after the 1 000 parents of 100 000 entities moved | B2-F06 |
| `world.children` | A parent of 10 000, then 20 000 children emptied, each given no parent: twice the children take four times as long while it is quadratic | G27 |
| `world.placements` | A scene file that places a prefab 2 000, then 8 000 times, each with a change, resolved | G26 |
| `world.sky` | Frames built around a sun that does not move, the sun set where it is at each fixed step, at the fractions of a step at which a display of 144 Hz shows a game stepping at 60 Hz, after a first frame that makes the sky's environment: the frames that made it again | G13 |
| `loading.batch` | A load of meshes started, on the calling thread, with a manifest of 10 000, then 100 000 assets | G25, B4-F03 |
| `loading.zone` | The meshes of *Hunna Wood*'s zone, cooked in `target/cooked/hunnawood`, each placed once on a grid and drawn by day, under a sun that casts shadows, while they load with their materials and textures, then once loaded: the time from the opening of the cooked zone until a frame draws them whole, the render CPU during the load and after. The zone's bytes, not its scene, whose components only the game registers. On request only, since it needs the zone fetched and cooked, as [*Hunna Wood*'s README](../games/hunnawood/README.md) says | B4-F01, B1-F06 |

The last column names the findings of [`docs/PLAN.md`](../docs/PLAN.md) whose changes the
benchmark measures before and after.

## Reading the figures

- On lavapipe, which renders on the CPU, `Renderer::render`'s CPU time holds the GPU work of the
  frame's earlier submissions, which lavapipe runs one after the other
  ([`RenderStats::cpu_time`](../crates/slime-render/src/renderer.rs)): it is not a CPU time
  there. `world.transforms.100k`, `world.children`, `world.placements` and `loading.batch` do not
  touch the GPU; the others draw, or make objects on the GPU. On lavapipe, what a draw costs is
  not in what it binds: `render.unseen.10k` shows no gain where a GPU's driver may.
- A frame "until the GPU finished it" includes the wait of `Gpu::wait_idle`, which wakes up to 1 ms
  after the GPU finished: on a GPU whose frames take a few milliseconds, the times of the passes
  on the GPU are the figures to compare. Each frame reports the latest time of each pass that came
  back from the GPU, a few frames old. Their sum leaves out the gaps between the frame's
  submissions, in which the GPU waits: it is the GPU's work for the frame, not a frame time
  ([`GpuTimer`](../crates/slime-gfx/src/timer.rs)).
- The figures of lavapipe are not those of a GPU: the gains of the renderer are measured on a GPU,
  in a section of its own.
- A figure compares with a figure of the same machine, driver and build only.
- The benchmarks measure; they check nothing, and fail only when a scenario cannot run.

## What it must not do

- Reach wgpu, or any API that a game cannot reach: it measures what a game gets.
- Run in `scripts/check.sh` or CI, whose shared machines give figures that vary from one run to
  the next; CI builds it with the workspace, so that it keeps compiling.
