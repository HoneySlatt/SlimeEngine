# Measurements

What the engine's builds and benchmarks measured, one section per milestone or machine, newest last
([ADR-0104](adr/0104-the-build-ci-and-measurements.md)). Each section names its commit, its date and
its machine, which a figure compares with only: the same benchmark on the same machine, driver and
build. A change that claims a gain shows the section before it and the section after it.

[`scripts/measure.sh`](../scripts/measure.sh) writes a section:

```sh
VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" scripts/measure.sh "<milestone>, lavapipe" >> docs/measurements.md
scripts/measure.sh "<milestone>, <GPU>" >> docs/measurements.md                # on a machine's own GPU
```

It measures the build of the workspace and of its tests from an empty target directory, with the
default debug settings, the run of the workspace's tests, and the benchmarks of
[`slime-benches`](../benches/README.md), which says what each scenario is and how its figures are
read. The figures that the plan's known debt and the ADRs give were measured once; this file holds
those that are measured again.

## P5, lavapipe

Commit `2d71729`, on 2026-10-19.

| Build | Measure | Value |
|---|---|---|
| `cargo build --workspace` | from an empty target directory | 113.1 s |
| `cargo test --workspace --no-run` | after it | 64.6 s |
| `cargo test --workspace --no-run` | libraries compiled again | 0 |
| target directory | after both | 8.18 GB |
| `cargo test --workspace` | its tests run, once built, doc tests included | 78.6 s |

4 logical CPUs, linux on x86_64, GPU `llvmpipe (LLVM 21.1.8, 256 bits)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `render.instances.10k` | CPU of `Renderer::render`, median of 30 | 275.14 ms |
| `render.instances.10k` | CPU of `Renderer::render`, 90th percentile of 30 | 318.60 ms |
| `render.instances.10k` | frame, until the GPU finished it, median of 30 | 301.15 ms |
| `render.instances.10k` | frame, until the GPU finished it, 90th percentile of 30 | 345.61 ms |
| `render.instances.10k` | scene pass on the GPU, median of what 30 frames reported | 139.41 ms |
| `render.instances.10k` | draw calls: scene, shadow cascades | 10000, 1715 |
| `render.shaders` | pipelines made again, shaders unchanged, median of 5 | 55.48 ms |
| `render.shaders` | pipelines made again after an edit of the scene's shader, median of 5 | 57.10 ms |
| `world.frame.100k` | `FrameBuilder::build`, median of 20 | 23.11 ms |
| `world.frame.100k` | `FrameBuilder::build`, 90th percentile of 20 | 25.86 ms |
| `world.frame.100k` | instances in a frame | 100000 |
| `world.transforms.100k` | `World::update_transforms` after 1 000 parents moved, median of 20 | 5.32 ms |
| `world.transforms.100k` | `World::update_transforms` after 1 000 parents moved, 90th percentile of 20 | 5.78 ms |
| `world.children` | a parent of 10000 children emptied, once | 49.70 ms |
| `world.children` | a parent of 20000 children emptied, once | 202.27 ms |
| `world.placements` | 2000 placements resolved, median of 3 | 47.95 ms |
| `world.placements` | 8000 placements resolved, median of 3 | 321.72 ms |
| `world.sky` | `FrameBuilder::build`, median of 24 | 73.04 ms |
| `world.sky` | `FrameBuilder::build`, slowest of 24 | 99.25 ms |
| `world.sky` | frames that made the sky again, longer than 1 ms, of 24 | 14 |
| `loading.batch` | a load started, the manifest of 10000 assets, median of 5 | 4.68 ms |
| `loading.batch` | a load started, the manifest of 100000 assets, median of 5 | 73.79 ms |
| `loading.zone` | cooked zone opened | 144.9 µs |
| `loading.zone` | meshes placed | 73 |
| `loading.zone` | first frame that draws them whole, from the opening | 1912.54 ms |
| `loading.zone` | CPU of `Renderer::render` while they load, median of 7 | 51.41 ms |
| `loading.zone` | CPU of `Renderer::render` while they load, 90th percentile of 7 | 73.15 ms |
| `loading.zone` | CPU of `Renderer::render` once they are loaded, median of 5 | 828.18 ms |
| `loading.zone` | CPU of `Renderer::render` once they are loaded, 90th percentile of 5 | 1131.44 ms |

## P4, lavapipe

Commit `0a5c7da`, on 2026-10-20.

| Build | Measure | Value |
|---|---|---|
| `cargo build --workspace` | from an empty target directory | 123.4 s |
| `cargo test --workspace --no-run` | after it | 70.1 s |
| `cargo test --workspace --no-run` | libraries compiled again | 0 |
| target directory | after both | 8.20 GB |
| `cargo test --workspace` | its tests run, once built, doc tests included | 82.5 s |

4 logical CPUs, linux on x86_64, GPU `llvmpipe (LLVM 21.1.8, 256 bits)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `render.instances.10k` | CPU of `Renderer::render`, median of 30 | 295.74 ms |
| `render.instances.10k` | CPU of `Renderer::render`, 90th percentile of 30 | 318.98 ms |
| `render.instances.10k` | frame, until the GPU finished it, median of 30 | 323.98 ms |
| `render.instances.10k` | frame, until the GPU finished it, 90th percentile of 30 | 352.96 ms |
| `render.instances.10k` | scene pass on the GPU, median of what 30 frames reported | 154.56 ms |
| `render.instances.10k` | draw calls: scene, shadow cascades | 10000, 1715 |
| `render.unseen.10k` | CPU of `Renderer::render`, median of 30 | 230.77 ms |
| `render.unseen.10k` | CPU of `Renderer::render`, 90th percentile of 30 | 262.63 ms |
| `render.unseen.10k` | frame, until the GPU finished it, median of 30 | 256.34 ms |
| `render.unseen.10k` | frame, until the GPU finished it, 90th percentile of 30 | 287.85 ms |
| `render.unseen.10k` | scene pass on the GPU, median of what 30 frames reported | 78.23 ms |
| `render.unseen.10k` | draw calls: scene, shadow cascades | 10000, 2447 |
| `render.shaders` | pipelines made again, shaders unchanged, median of 5 | 64.31 ms |
| `render.shaders` | pipelines made again after an edit of the scene's shader, median of 5 | 58.20 ms |
| `world.frame.100k` | `FrameBuilder::build`, median of 20 | 17.02 ms |
| `world.frame.100k` | `FrameBuilder::build`, 90th percentile of 20 | 18.40 ms |
| `world.frame.100k` | instances in a frame | 100000 |
| `world.transforms.100k` | `World::update_transforms` after 1 000 parents moved, median of 20 | 5.08 ms |
| `world.transforms.100k` | `World::update_transforms` after 1 000 parents moved, 90th percentile of 20 | 5.22 ms |
| `world.children` | a parent of 10000 children emptied, once | 169.9 µs |
| `world.children` | a parent of 20000 children emptied, once | 200.8 µs |
| `world.placements` | 2000 placements resolved, median of 3 | 41.01 ms |
| `world.placements` | 8000 placements resolved, median of 3 | 219.15 ms |
| `world.sky` | `FrameBuilder::build`, median of 24 | 0.2 µs |
| `world.sky` | `FrameBuilder::build`, slowest of 24 | 12.6 µs |
| `world.sky` | frames that made the sky again, longer than 1 ms, of 24 | 0 |
| `loading.batch` | a load started, the manifest of 10000 assets, median of 5 | 29.4 µs |
| `loading.batch` | a load started, the manifest of 100000 assets, median of 5 | 19.4 µs |
| `loading.zone` | cooked zone opened | 118.9 µs |
| `loading.zone` | meshes placed | 73 |
| `loading.zone` | first frame that draws them whole, from the opening | 1753.45 ms |
| `loading.zone` | CPU of `Renderer::render` while they load, median of 5 | 55.92 ms |
| `loading.zone` | CPU of `Renderer::render` while they load, 90th percentile of 5 | 76.51 ms |
| `loading.zone` | CPU of `Renderer::render` once they are loaded, median of 5 | 767.79 ms |
| `loading.zone` | CPU of `Renderer::render` once they are loaded, 90th percentile of 5 | 1155.19 ms |
