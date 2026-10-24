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

## P5, RX 6900 XT

Commit `b55721c`, on 2026-10-20.

| Build | Measure | Value |
|---|---|---|
| `cargo build --workspace` | from an empty target directory | 34.6 s |
| `cargo test --workspace --no-run` | after it | 24.8 s |
| `cargo test --workspace --no-run` | libraries compiled again | 0 |
| target directory | after both | 8.19 GB |
| `cargo test --workspace` | its tests run, once built, doc tests included | 51.3 s |

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `render.instances.10k` | CPU of `Renderer::render`, median of 30 | 17.90 ms |
| `render.instances.10k` | CPU of `Renderer::render`, 90th percentile of 30 | 18.53 ms |
| `render.instances.10k` | frame, until the GPU finished it, median of 30 | 20.37 ms |
| `render.instances.10k` | frame, until the GPU finished it, 90th percentile of 30 | 21.33 ms |
| `render.instances.10k` | scene pass on the GPU, median of what 30 frames reported | 934.2 µs |
| `render.instances.10k` | draw calls: scene, shadow cascades | 10000, 1715 |
| `render.instances.100k` | CPU of `Renderer::render`, median of 8 | 131.20 ms |
| `render.instances.100k` | CPU of `Renderer::render`, 90th percentile of 8 | 134.26 ms |
| `render.instances.100k` | frame, until the GPU finished it, median of 8 | 165.99 ms |
| `render.instances.100k` | frame, until the GPU finished it, 90th percentile of 8 | 168.63 ms |
| `render.instances.100k` | scene pass on the GPU, median of what 8 frames reported | 27.43 ms |
| `render.instances.100k` | draw calls: scene, shadow cascades | 100000, 1331 |
| `render.shaders` | pipelines made again, shaders unchanged, median of 5 | 4.79 ms |
| `render.shaders` | pipelines made again after an edit of the scene's shader, median of 5 | 5.56 ms |
| `world.frame.100k` | `FrameBuilder::build`, median of 20 | 20.05 ms |
| `world.frame.100k` | `FrameBuilder::build`, 90th percentile of 20 | 20.67 ms |
| `world.frame.100k` | instances in a frame | 100000 |
| `world.transforms.100k` | `World::update_transforms` after 1 000 parents moved, median of 20 | 3.63 ms |
| `world.transforms.100k` | `World::update_transforms` after 1 000 parents moved, 90th percentile of 20 | 3.75 ms |
| `world.children` | a parent of 10000 children emptied, once | 30.36 ms |
| `world.children` | a parent of 20000 children emptied, once | 124.35 ms |
| `world.placements` | 2000 placements resolved, median of 3 | 37.82 ms |
| `world.placements` | 8000 placements resolved, median of 3 | 257.25 ms |
| `world.sky` | `FrameBuilder::build`, median of 24 | 28.18 ms |
| `world.sky` | `FrameBuilder::build`, slowest of 24 | 32.47 ms |
| `world.sky` | frames that made the sky again, longer than 1 ms, of 24 | 14 |
| `loading.batch` | a load started, the manifest of 10000 assets, median of 5 | 4.02 ms |
| `loading.batch` | a load started, the manifest of 100000 assets, median of 5 | 51.95 ms |
| `loading.zone` | cooked zone opened | 664.3 µs |
| `loading.zone` | meshes placed | 73 |
| `loading.zone` | first frame that draws them whole, from the opening | 220.03 ms |
| `loading.zone` | CPU of `Renderer::render` while they load, median of 92 | 956.5 µs |
| `loading.zone` | CPU of `Renderer::render` while they load, 90th percentile of 92 | 2.39 ms |
| `loading.zone` | CPU of `Renderer::render` once they are loaded, median of 5 | 639.2 µs |
| `loading.zone` | CPU of `Renderer::render` once they are loaded, 90th percentile of 5 | 1.38 ms |

## P4, RX 6900 XT

Commit `5b7b05d`, on 2026-10-20.

| Build | Measure | Value |
|---|---|---|
| `cargo build --workspace` | from an empty target directory | 34.0 s |
| `cargo test --workspace --no-run` | after it | 24.3 s |
| `cargo test --workspace --no-run` | libraries compiled again | 0 |
| target directory | after both | 8.20 GB |
| `cargo test --workspace` | its tests run, once built, doc tests included | 50.4 s |

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `render.instances.10k` | CPU of `Renderer::render`, median of 30 | 8.76 ms |
| `render.instances.10k` | CPU of `Renderer::render`, 90th percentile of 30 | 9.83 ms |
| `render.instances.10k` | frame, until the GPU finished it, median of 30 | 9.47 ms |
| `render.instances.10k` | frame, until the GPU finished it, 90th percentile of 30 | 10.50 ms |
| `render.instances.10k` | scene pass on the GPU, median of what 30 frames reported | 151.6 µs |
| `render.instances.10k` | draw calls: scene, shadow cascades | 10000, 1715 |
| `render.unseen.10k` | CPU of `Renderer::render`, median of 30 | 8.76 ms |
| `render.unseen.10k` | CPU of `Renderer::render`, 90th percentile of 30 | 9.40 ms |
| `render.unseen.10k` | frame, until the GPU finished it, median of 30 | 9.25 ms |
| `render.unseen.10k` | frame, until the GPU finished it, 90th percentile of 30 | 9.79 ms |
| `render.unseen.10k` | scene pass on the GPU, median of what 30 frames reported | 107.6 µs |
| `render.unseen.10k` | draw calls: scene, shadow cascades | 10000, 2447 |
| `render.instances.100k` | CPU of `Renderer::render`, median of 8 | 74.27 ms |
| `render.instances.100k` | CPU of `Renderer::render`, 90th percentile of 8 | 74.75 ms |
| `render.instances.100k` | frame, until the GPU finished it, median of 8 | 77.86 ms |
| `render.instances.100k` | frame, until the GPU finished it, 90th percentile of 8 | 78.52 ms |
| `render.instances.100k` | scene pass on the GPU, median of what 8 frames reported | 1.20 ms |
| `render.instances.100k` | draw calls: scene, shadow cascades | 100000, 1331 |
| `render.shaders` | pipelines made again, shaders unchanged, median of 5 | 4.20 ms |
| `render.shaders` | pipelines made again after an edit of the scene's shader, median of 5 | 4.19 ms |
| `world.frame.100k` | `FrameBuilder::build`, median of 20 | 11.72 ms |
| `world.frame.100k` | `FrameBuilder::build`, 90th percentile of 20 | 13.04 ms |
| `world.frame.100k` | instances in a frame | 100000 |
| `world.transforms.100k` | `World::update_transforms` after 1 000 parents moved, median of 20 | 3.72 ms |
| `world.transforms.100k` | `World::update_transforms` after 1 000 parents moved, 90th percentile of 20 | 3.84 ms |
| `world.children` | a parent of 10000 children emptied, once | 91.9 µs |
| `world.children` | a parent of 20000 children emptied, once | 188.8 µs |
| `world.placements` | 2000 placements resolved, median of 3 | 31.97 ms |
| `world.placements` | 8000 placements resolved, median of 3 | 161.18 ms |
| `world.sky` | `FrameBuilder::build`, median of 24 | 0.1 µs |
| `world.sky` | `FrameBuilder::build`, slowest of 24 | 4.0 µs |
| `world.sky` | frames that made the sky again, longer than 1 ms, of 24 | 0 |
| `loading.batch` | a load started, the manifest of 10000 assets, median of 5 | 18.5 µs |
| `loading.batch` | a load started, the manifest of 100000 assets, median of 5 | 17.8 µs |
| `loading.zone` | cooked zone opened | 384.0 µs |
| `loading.zone` | meshes placed | 73 |
| `loading.zone` | first frame that draws them whole, from the opening | 161.17 ms |
| `loading.zone` | CPU of `Renderer::render` while they load, median of 83 | 774.5 µs |
| `loading.zone` | CPU of `Renderer::render` while they load, 90th percentile of 83 | 2.24 ms |
| `loading.zone` | CPU of `Renderer::render` once they are loaded, median of 5 | 538.9 µs |
| `loading.zone` | CPU of `Renderer::render` once they are loaded, 90th percentile of 5 | 1.41 ms |

## M6a.1, before, RX 6900 XT

Commit `2ecb5f3`, on 2026-10-20: the renderer before frustum culling and instancing, its
benchmarks at the hardware tiers' resolutions. The build is not measured in this section, nor in
the three that follow: the machine was installing a game, which kept about one logical CPU and the
disk busy. These figures vary from one run to the next: a second run gave, for
`render.instances.10k`, 8.88, 11.16 and 13.72 ms of CPU at the three sizes, for
`render.unseen.10k` 16.46, 14.49 and 15.18 ms, and for `render.instances.100k` 73.62, 74.24 and
73.19 ms.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `render.instances.10k` | CPU of `Renderer::render` at 1280 × 800, median of 30 | 10.05 ms |
| `render.instances.10k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 30 | 18.41 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1280 × 800, median of 30 | 10.80 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 30 | 18.66 ms |
| `render.instances.10k` | scene pass on the GPU at 1280 × 800, median of what 30 frames reported | 153.3 µs |
| `render.instances.10k` | passes on the GPU, summed at 1280 × 800, median of what 30 frames reported | 267.2 µs |
| `render.instances.10k` | draw calls at 1280 × 800: scene, shadow cascades | 10000, 1484 |
| `render.instances.10k` | CPU of `Renderer::render` at 1920 × 1080, median of 30 | 17.04 ms |
| `render.instances.10k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 30 | 18.98 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1920 × 1080, median of 30 | 17.24 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 30 | 19.19 ms |
| `render.instances.10k` | scene pass on the GPU at 1920 × 1080, median of what 30 frames reported | 177.2 µs |
| `render.instances.10k` | passes on the GPU, summed at 1920 × 1080, median of what 30 frames reported | 349.0 µs |
| `render.instances.10k` | draw calls at 1920 × 1080: scene, shadow cascades | 10000, 1715 |
| `render.instances.10k` | CPU of `Renderer::render` at 2560 × 1440, median of 30 | 17.33 ms |
| `render.instances.10k` | CPU of `Renderer::render` at 2560 × 1440, 90th percentile of 30 | 18.46 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 2560 × 1440, median of 30 | 17.52 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 2560 × 1440, 90th percentile of 30 | 18.65 ms |
| `render.instances.10k` | scene pass on the GPU at 2560 × 1440, median of what 30 frames reported | 188.4 µs |
| `render.instances.10k` | passes on the GPU, summed at 2560 × 1440, median of what 30 frames reported | 442.6 µs |
| `render.instances.10k` | draw calls at 2560 × 1440: scene, shadow cascades | 10000, 1715 |
| `render.unseen.10k` | CPU of `Renderer::render` at 1280 × 800, median of 30 | 20.62 ms |
| `render.unseen.10k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 30 | 24.29 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1280 × 800, median of 30 | 20.83 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 30 | 25.11 ms |
| `render.unseen.10k` | scene pass on the GPU at 1280 × 800, median of what 30 frames reported | 169.1 µs |
| `render.unseen.10k` | passes on the GPU, summed at 1280 × 800, median of what 30 frames reported | 286.1 µs |
| `render.unseen.10k` | draw calls at 1280 × 800: scene, shadow cascades | 10000, 2150 |
| `render.unseen.10k` | CPU of `Renderer::render` at 1920 × 1080, median of 30 | 23.42 ms |
| `render.unseen.10k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 30 | 30.53 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1920 × 1080, median of 30 | 24.11 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 30 | 30.96 ms |
| `render.unseen.10k` | scene pass on the GPU at 1920 × 1080, median of what 30 frames reported | 174.3 µs |
| `render.unseen.10k` | passes on the GPU, summed at 1920 × 1080, median of what 30 frames reported | 356.5 µs |
| `render.unseen.10k` | draw calls at 1920 × 1080: scene, shadow cascades | 10000, 2447 |
| `render.unseen.10k` | CPU of `Renderer::render` at 2560 × 1440, median of 30 | 19.02 ms |
| `render.unseen.10k` | CPU of `Renderer::render` at 2560 × 1440, 90th percentile of 30 | 23.30 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 2560 × 1440, median of 30 | 19.38 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 2560 × 1440, 90th percentile of 30 | 23.89 ms |
| `render.unseen.10k` | scene pass on the GPU at 2560 × 1440, median of what 30 frames reported | 168.8 µs |
| `render.unseen.10k` | passes on the GPU, summed at 2560 × 1440, median of what 30 frames reported | 423.8 µs |
| `render.unseen.10k` | draw calls at 2560 × 1440: scene, shadow cascades | 10000, 2447 |
| `render.instances.100k` | CPU of `Renderer::render` at 1280 × 800, median of 8 | 84.46 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 8 | 94.69 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1280 × 800, median of 8 | 88.63 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 8 | 94.91 ms |
| `render.instances.100k` | scene pass on the GPU at 1280 × 800, median of what 8 frames reported | 1.84 ms |
| `render.instances.100k` | passes on the GPU, summed at 1280 × 800, median of what 8 frames reported | 1.97 ms |
| `render.instances.100k` | draw calls at 1280 × 800: scene, shadow cascades | 100000, 1154 |
| `render.instances.100k` | CPU of `Renderer::render` at 1920 × 1080, median of 8 | 79.94 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 8 | 84.60 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1920 × 1080, median of 8 | 80.89 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 8 | 84.94 ms |
| `render.instances.100k` | scene pass on the GPU at 1920 × 1080, median of what 8 frames reported | 2.01 ms |
| `render.instances.100k` | passes on the GPU, summed at 1920 × 1080, median of what 8 frames reported | 2.22 ms |
| `render.instances.100k` | draw calls at 1920 × 1080: scene, shadow cascades | 100000, 1331 |
| `render.instances.100k` | CPU of `Renderer::render` at 2560 × 1440, median of 8 | 92.63 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 2560 × 1440, 90th percentile of 8 | 93.79 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 2560 × 1440, median of 8 | 92.87 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 2560 × 1440, 90th percentile of 8 | 94.68 ms |
| `render.instances.100k` | scene pass on the GPU at 2560 × 1440, median of what 8 frames reported | 1.94 ms |
| `render.instances.100k` | passes on the GPU, summed at 2560 × 1440, median of what 8 frames reported | 2.25 ms |
| `render.instances.100k` | draw calls at 2560 × 1440: scene, shadow cascades | 100000, 1331 |
| `world.frame.100k` | `FrameBuilder::build`, median of 20 | 13.10 ms |
| `world.frame.100k` | `FrameBuilder::build`, 90th percentile of 20 | 13.53 ms |
| `world.frame.100k` | instances in a frame | 100000 |

## M6a.1, RX 6900 XT

Commit `5e2267d`, on 2026-10-20, measured as the section above. An earlier run, of the code before
the last changes of its review, gave, for `render.instances.10k`, 2.16, 2.27 and 2.48 ms of CPU at
the three sizes, for `render.unseen.10k` 1.34, 1.39 and 1.35 ms, and for `render.instances.100k`
17.36, 19.54 and 21.15 ms; a second one, 2.23, 2.58 and 2.10 ms, 1.43, 1.39 and 1.36 ms, and 18.97,
19.77 and 19.71 ms.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `render.instances.10k` | CPU of `Renderer::render` at 1280 × 800, median of 30 | 2.04 ms |
| `render.instances.10k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 30 | 2.41 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1280 × 800, median of 30 | 2.40 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 30 | 3.22 ms |
| `render.instances.10k` | scene pass on the GPU at 1280 × 800, median of what 30 frames reported | 39.0 µs |
| `render.instances.10k` | passes on the GPU, summed at 1280 × 800, median of what 30 frames reported | 150.2 µs |
| `render.instances.10k` | draw calls at 1280 × 800: scene, shadow cascades | 8, 3 |
| `render.instances.10k` | instances that the scene pass drew at 1280 × 800 | 9343 |
| `render.instances.10k` | CPU of `Renderer::render` at 1920 × 1080, median of 30 | 2.19 ms |
| `render.instances.10k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 30 | 2.66 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1920 × 1080, median of 30 | 2.51 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 30 | 2.85 ms |
| `render.instances.10k` | scene pass on the GPU at 1920 × 1080, median of what 30 frames reported | 52.4 µs |
| `render.instances.10k` | passes on the GPU, summed at 1920 × 1080, median of what 30 frames reported | 219.0 µs |
| `render.instances.10k` | draw calls at 1920 × 1080: scene, shadow cascades | 8, 3 |
| `render.instances.10k` | instances that the scene pass drew at 1920 × 1080 | 9517 |
| `render.instances.10k` | CPU of `Renderer::render` at 2560 × 1440, median of 30 | 2.04 ms |
| `render.instances.10k` | CPU of `Renderer::render` at 2560 × 1440, 90th percentile of 30 | 2.28 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 2560 × 1440, median of 30 | 2.58 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 2560 × 1440, 90th percentile of 30 | 2.70 ms |
| `render.instances.10k` | scene pass on the GPU at 2560 × 1440, median of what 30 frames reported | 73.0 µs |
| `render.instances.10k` | passes on the GPU, summed at 2560 × 1440, median of what 30 frames reported | 320.3 µs |
| `render.instances.10k` | draw calls at 2560 × 1440: scene, shadow cascades | 8, 3 |
| `render.instances.10k` | instances that the scene pass drew at 2560 × 1440 | 9517 |
| `render.unseen.10k` | CPU of `Renderer::render` at 1280 × 800, median of 30 | 1.34 ms |
| `render.unseen.10k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 30 | 1.61 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1280 × 800, median of 30 | 1.53 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 30 | 1.79 ms |
| `render.unseen.10k` | scene pass on the GPU at 1280 × 800, median of what 30 frames reported | 2.3 µs |
| `render.unseen.10k` | passes on the GPU, summed at 1280 × 800, median of what 30 frames reported | 112.8 µs |
| `render.unseen.10k` | draw calls at 1280 × 800: scene, shadow cascades | 0, 4 |
| `render.unseen.10k` | instances that the scene pass drew at 1280 × 800 | 0 |
| `render.unseen.10k` | CPU of `Renderer::render` at 1920 × 1080, median of 30 | 1.43 ms |
| `render.unseen.10k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 30 | 1.67 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1920 × 1080, median of 30 | 1.61 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 30 | 1.86 ms |
| `render.unseen.10k` | scene pass on the GPU at 1920 × 1080, median of what 30 frames reported | 2.4 µs |
| `render.unseen.10k` | passes on the GPU, summed at 1920 × 1080, median of what 30 frames reported | 167.0 µs |
| `render.unseen.10k` | draw calls at 1920 × 1080: scene, shadow cascades | 0, 4 |
| `render.unseen.10k` | instances that the scene pass drew at 1920 × 1080 | 0 |
| `render.unseen.10k` | CPU of `Renderer::render` at 2560 × 1440, median of 30 | 1.38 ms |
| `render.unseen.10k` | CPU of `Renderer::render` at 2560 × 1440, 90th percentile of 30 | 1.42 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 2560 × 1440, median of 30 | 1.73 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 2560 × 1440, 90th percentile of 30 | 1.77 ms |
| `render.unseen.10k` | scene pass on the GPU at 2560 × 1440, median of what 30 frames reported | 2.4 µs |
| `render.unseen.10k` | passes on the GPU, summed at 2560 × 1440, median of what 30 frames reported | 245.9 µs |
| `render.unseen.10k` | draw calls at 2560 × 1440: scene, shadow cascades | 0, 4 |
| `render.unseen.10k` | instances that the scene pass drew at 2560 × 1440 | 0 |
| `render.instances.100k` | CPU of `Renderer::render` at 1280 × 800, median of 8 | 19.25 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 8 | 19.60 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1280 × 800, median of 8 | 20.44 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 8 | 20.59 ms |
| `render.instances.100k` | scene pass on the GPU at 1280 × 800, median of what 8 frames reported | 143.3 µs |
| `render.instances.100k` | passes on the GPU, summed at 1280 × 800, median of what 8 frames reported | 266.4 µs |
| `render.instances.100k` | draw calls at 1280 × 800: scene, shadow cascades | 8, 2 |
| `render.instances.100k` | instances that the scene pass drew at 1280 × 800 | 93539 |
| `render.instances.100k` | CPU of `Renderer::render` at 1920 × 1080, median of 8 | 18.17 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 8 | 19.90 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1920 × 1080, median of 8 | 19.35 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 8 | 20.40 ms |
| `render.instances.100k` | scene pass on the GPU at 1920 × 1080, median of what 8 frames reported | 164.0 µs |
| `render.instances.100k` | passes on the GPU, summed at 1920 × 1080, median of what 8 frames reported | 360.0 µs |
| `render.instances.100k` | draw calls at 1920 × 1080: scene, shadow cascades | 8, 2 |
| `render.instances.100k` | instances that the scene pass drew at 1920 × 1080 | 95310 |
| `render.instances.100k` | CPU of `Renderer::render` at 2560 × 1440, median of 8 | 19.09 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 2560 × 1440, 90th percentile of 8 | 19.25 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 2560 × 1440, median of 8 | 20.28 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 2560 × 1440, 90th percentile of 8 | 20.49 ms |
| `render.instances.100k` | scene pass on the GPU at 2560 × 1440, median of what 8 frames reported | 193.6 µs |
| `render.instances.100k` | passes on the GPU, summed at 2560 × 1440, median of what 8 frames reported | 495.4 µs |
| `render.instances.100k` | draw calls at 2560 × 1440: scene, shadow cascades | 8, 2 |
| `render.instances.100k` | instances that the scene pass drew at 2560 × 1440 | 95310 |
| `world.frame.100k` | `FrameBuilder::build`, median of 20 | 13.26 ms |
| `world.frame.100k` | `FrameBuilder::build`, 90th percentile of 20 | 13.60 ms |
| `world.frame.100k` | instances in a frame | 100000 |

The budgets of [ADR-0114](adr/0114-hardware-tiers-and-the-gpus-limits.md), scaled to this GPU: the
GPU takes at most about 85 % of a frame, times the tier's ratio to the RX 6900 XT, a tenth for the
handheld tier, a half for the modest one, one for the high one; the main thread, whose work holds
`Renderer::render`'s, at most about 70 % of a frame, which ADR-0114 does not scale.

| Tier, size | GPU budget, a realistic game / a competitive shooter | Passes on the GPU, 10 000 / 100 000 cubes | Main thread's budget, 60 / 144 frames a second | CPU of `Renderer::render`, 10 000 / 100 000 cubes |
|---|---|---|---|---|
| Handheld, 1280 × 800 | 2.8 ms at 30 / 1.4 ms at 60 frames a second | 0.15 / 0.27 ms | 23.3 ms at 30 / 11.7 ms at 60 | 2.0 / 19.3 ms |
| Modest, 1920 × 1080 | 7.1 ms at 60 / 3.0 ms at 144 | 0.22 / 0.36 ms | 11.7 / 4.9 ms | 2.2 / 18.2 ms |
| High, 2560 × 1440 | 14.2 ms at 60 / 5.9 ms at 144 | 0.32 / 0.50 ms | 11.7 / 4.9 ms | 2.0 / 19.1 ms |

The cubes' GPU work is within every budget, a fifth of the smallest at most, that of the handheld
tier's shooter. The CPU of 10 000 cubes is within the main thread's budget at 144 frames a second,
less than half of it; that of 100 000 is beyond the budget at 60 by a half to two thirds, and within
it only at the handheld tier's 30 frames a second, on this machine's CPU: what each instance costs
on the CPU at each frame, which [`PLAN.md`](PLAN.md)'s known debt says. `FrameBuilder::build` over
100 000 entities (`world.frame.100k`) still costs less than drawing them, so that
[ADR-0111](adr/0111-the-frame-contract.md)'s condition for building the frame from what changed
does not hold yet.

## M6a.1, before, lavapipe

Commit `2ecb5f3`, on 2026-10-20, measured as the RX 6900 XT's section before.
`render.instances.100k` is not run: lavapipe (Mesa 26.2.4) crashed executing a frame of 100 000
draws.

16 logical CPUs, linux on x86_64, GPU `llvmpipe (LLVM 21.1.8, 256 bits)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `render.instances.10k` | CPU of `Renderer::render` at 1280 × 800, median of 30 | 223.30 ms |
| `render.instances.10k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 30 | 237.38 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1280 × 800, median of 30 | 232.55 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 30 | 247.32 ms |
| `render.instances.10k` | scene pass on the GPU at 1280 × 800, median of what 30 frames reported | 129.16 ms |
| `render.instances.10k` | passes on the GPU, summed at 1280 × 800, median of what 30 frames reported | 184.67 ms |
| `render.instances.10k` | draw calls at 1280 × 800: scene, shadow cascades | 10000, 1484 |
| `render.instances.10k` | CPU of `Renderer::render` at 1920 × 1080, median of 30 | 255.05 ms |
| `render.instances.10k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 30 | 290.12 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1920 × 1080, median of 30 | 272.13 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 30 | 310.11 ms |
| `render.instances.10k` | scene pass on the GPU at 1920 × 1080, median of what 30 frames reported | 141.23 ms |
| `render.instances.10k` | passes on the GPU, summed at 1920 × 1080, median of what 30 frames reported | 219.54 ms |
| `render.instances.10k` | draw calls at 1920 × 1080: scene, shadow cascades | 10000, 1715 |
| `render.instances.10k` | CPU of `Renderer::render` at 2560 × 1440, median of 30 | 346.43 ms |
| `render.instances.10k` | CPU of `Renderer::render` at 2560 × 1440, 90th percentile of 30 | 621.52 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 2560 × 1440, median of 30 | 384.21 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 2560 × 1440, 90th percentile of 30 | 698.13 ms |
| `render.instances.10k` | scene pass on the GPU at 2560 × 1440, median of what 30 frames reported | 185.49 ms |
| `render.instances.10k` | passes on the GPU, summed at 2560 × 1440, median of what 30 frames reported | 320.49 ms |
| `render.instances.10k` | draw calls at 2560 × 1440: scene, shadow cascades | 10000, 1715 |
| `render.unseen.10k` | CPU of `Renderer::render` at 1280 × 800, median of 30 | 200.84 ms |
| `render.unseen.10k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 30 | 228.57 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1280 × 800, median of 30 | 210.27 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 30 | 275.12 ms |
| `render.unseen.10k` | scene pass on the GPU at 1280 × 800, median of what 30 frames reported | 86.68 ms |
| `render.unseen.10k` | passes on the GPU, summed at 1280 × 800, median of what 30 frames reported | 157.12 ms |
| `render.unseen.10k` | draw calls at 1280 × 800: scene, shadow cascades | 10000, 2150 |
| `render.unseen.10k` | CPU of `Renderer::render` at 1920 × 1080, median of 30 | 233.65 ms |
| `render.unseen.10k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 30 | 262.69 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1920 × 1080, median of 30 | 252.20 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 30 | 290.01 ms |
| `render.unseen.10k` | scene pass on the GPU at 1920 × 1080, median of what 30 frames reported | 91.41 ms |
| `render.unseen.10k` | passes on the GPU, summed at 1920 × 1080, median of what 30 frames reported | 195.36 ms |
| `render.unseen.10k` | draw calls at 1920 × 1080: scene, shadow cascades | 10000, 2447 |
| `render.unseen.10k` | CPU of `Renderer::render` at 2560 × 1440, median of 30 | 268.00 ms |
| `render.unseen.10k` | CPU of `Renderer::render` at 2560 × 1440, 90th percentile of 30 | 312.99 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 2560 × 1440, median of 30 | 305.18 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 2560 × 1440, 90th percentile of 30 | 357.00 ms |
| `render.unseen.10k` | scene pass on the GPU at 2560 × 1440, median of what 30 frames reported | 99.25 ms |
| `render.unseen.10k` | passes on the GPU, summed at 2560 × 1440, median of what 30 frames reported | 240.57 ms |
| `render.unseen.10k` | draw calls at 2560 × 1440: scene, shadow cascades | 10000, 2447 |

## M6a.1, lavapipe

Commit `5e2267d`, on 2026-10-20, measured as the section above. Lavapipe runs
`render.instances.100k` now, its frame drawing the cubes with 10 draws. An earlier run, of the code
before the last changes of its review, gave, for `render.instances.10k`, 72.93, 125.13 and
159.51 ms of CPU at the three sizes, for `render.unseen.10k` 38.17, 73.54 and 96.02 ms, and for
`render.instances.100k` 389.76, 374.10 and 419.95 ms.

16 logical CPUs, linux on x86_64, GPU `llvmpipe (LLVM 21.1.8, 256 bits)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `render.instances.10k` | CPU of `Renderer::render` at 1280 × 800, median of 30 | 65.28 ms |
| `render.instances.10k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 30 | 68.04 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1280 × 800, median of 30 | 73.89 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 30 | 77.05 ms |
| `render.instances.10k` | scene pass on the GPU at 1280 × 800, median of what 30 frames reported | 39.63 ms |
| `render.instances.10k` | passes on the GPU, summed at 1280 × 800, median of what 30 frames reported | 70.10 ms |
| `render.instances.10k` | draw calls at 1280 × 800: scene, shadow cascades | 8, 3 |
| `render.instances.10k` | instances that the scene pass drew at 1280 × 800 | 9343 |
| `render.instances.10k` | CPU of `Renderer::render` at 1920 × 1080, median of 30 | 88.73 ms |
| `render.instances.10k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 30 | 90.62 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1920 × 1080, median of 30 | 103.80 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 30 | 106.58 ms |
| `render.instances.10k` | scene pass on the GPU at 1920 × 1080, median of what 30 frames reported | 50.02 ms |
| `render.instances.10k` | passes on the GPU, summed at 1920 × 1080, median of what 30 frames reported | 99.61 ms |
| `render.instances.10k` | draw calls at 1920 × 1080: scene, shadow cascades | 8, 3 |
| `render.instances.10k` | instances that the scene pass drew at 1920 × 1080 | 9517 |
| `render.instances.10k` | CPU of `Renderer::render` at 2560 × 1440, median of 30 | 126.15 ms |
| `render.instances.10k` | CPU of `Renderer::render` at 2560 × 1440, 90th percentile of 30 | 135.85 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 2560 × 1440, median of 30 | 155.57 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 2560 × 1440, 90th percentile of 30 | 166.14 ms |
| `render.instances.10k` | scene pass on the GPU at 2560 × 1440, median of what 30 frames reported | 68.45 ms |
| `render.instances.10k` | passes on the GPU, summed at 2560 × 1440, median of what 30 frames reported | 151.54 ms |
| `render.instances.10k` | draw calls at 2560 × 1440: scene, shadow cascades | 8, 3 |
| `render.instances.10k` | instances that the scene pass drew at 2560 × 1440 | 9517 |
| `render.unseen.10k` | CPU of `Renderer::render` at 1280 × 800, median of 30 | 33.42 ms |
| `render.unseen.10k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 30 | 37.38 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1280 × 800, median of 30 | 42.86 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 30 | 46.85 ms |
| `render.unseen.10k` | scene pass on the GPU at 1280 × 800, median of what 30 frames reported | 3.97 ms |
| `render.unseen.10k` | passes on the GPU, summed at 1280 × 800, median of what 30 frames reported | 39.57 ms |
| `render.unseen.10k` | draw calls at 1280 × 800: scene, shadow cascades | 0, 4 |
| `render.unseen.10k` | instances that the scene pass drew at 1280 × 800 | 0 |
| `render.unseen.10k` | CPU of `Renderer::render` at 1920 × 1080, median of 30 | 53.91 ms |
| `render.unseen.10k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 30 | 57.03 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1920 × 1080, median of 30 | 71.03 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 30 | 74.96 ms |
| `render.unseen.10k` | scene pass on the GPU at 1920 × 1080, median of what 30 frames reported | 8.63 ms |
| `render.unseen.10k` | passes on the GPU, summed at 1920 × 1080, median of what 30 frames reported | 67.62 ms |
| `render.unseen.10k` | draw calls at 1920 × 1080: scene, shadow cascades | 0, 4 |
| `render.unseen.10k` | instances that the scene pass drew at 1920 × 1080 | 0 |
| `render.unseen.10k` | CPU of `Renderer::render` at 2560 × 1440, median of 30 | 72.64 ms |
| `render.unseen.10k` | CPU of `Renderer::render` at 2560 × 1440, 90th percentile of 30 | 76.08 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 2560 × 1440, median of 30 | 102.14 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 2560 × 1440, 90th percentile of 30 | 104.96 ms |
| `render.unseen.10k` | scene pass on the GPU at 2560 × 1440, median of what 30 frames reported | 14.70 ms |
| `render.unseen.10k` | passes on the GPU, summed at 2560 × 1440, median of what 30 frames reported | 98.34 ms |
| `render.unseen.10k` | draw calls at 2560 × 1440: scene, shadow cascades | 0, 4 |
| `render.unseen.10k` | instances that the scene pass drew at 2560 × 1440 | 0 |
| `render.instances.100k` | CPU of `Renderer::render` at 1280 × 800, median of 8 | 325.68 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 8 | 330.51 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1280 × 800, median of 8 | 334.91 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 8 | 339.35 ms |
| `render.instances.100k` | scene pass on the GPU at 1280 × 800, median of what 8 frames reported | 282.87 ms |
| `render.instances.100k` | passes on the GPU, summed at 1280 × 800, median of what 8 frames reported | 312.79 ms |
| `render.instances.100k` | draw calls at 1280 × 800: scene, shadow cascades | 8, 2 |
| `render.instances.100k` | instances that the scene pass drew at 1280 × 800 | 93539 |
| `render.instances.100k` | CPU of `Renderer::render` at 1920 × 1080, median of 8 | 358.73 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 8 | 366.48 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1920 × 1080, median of 8 | 374.04 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 8 | 382.35 ms |
| `render.instances.100k` | scene pass on the GPU at 1920 × 1080, median of what 8 frames reported | 301.16 ms |
| `render.instances.100k` | passes on the GPU, summed at 1920 × 1080, median of what 8 frames reported | 351.00 ms |
| `render.instances.100k` | draw calls at 1920 × 1080: scene, shadow cascades | 8, 2 |
| `render.instances.100k` | instances that the scene pass drew at 1920 × 1080 | 95310 |
| `render.instances.100k` | CPU of `Renderer::render` at 2560 × 1440, median of 8 | 394.55 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 2560 × 1440, 90th percentile of 8 | 400.71 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 2560 × 1440, median of 8 | 423.06 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 2560 × 1440, 90th percentile of 8 | 430.06 ms |
| `render.instances.100k` | scene pass on the GPU at 2560 × 1440, median of what 8 frames reported | 320.31 ms |
| `render.instances.100k` | passes on the GPU, summed at 2560 × 1440, median of what 8 frames reported | 399.58 ms |
| `render.instances.100k` | draw calls at 2560 × 1440: scene, shadow cascades | 8, 2 |
| `render.instances.100k` | instances that the scene pass drew at 2560 × 1440 | 95310 |


## M6a.2, before, RX 6900 XT

Commit `b191845`, on 2026-10-21: the loads before the engine's task pool and the uploader, with the
benchmark of textures of 4096² and the warm-up frame that M6a.2 added first. The build is not
measured in this section, nor in the next. A second run gave, for `loading.textures`, its meshes
whole after 259.85 ms, the longest frame 26.17 ms, the render CPU at the 90th percentile 766.3 µs
while they load; for `loading.zone`, 77.33 ms, 6.25 ms and 1.76 ms. Runs earlier the same day, on
a machine that ran other programs, gave longest frames of 29.78 and 31.27 ms for the textures.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `loading.textures` | cooked data opened | 37.9 µs |
| `loading.textures` | meshes placed | 16 |
| `loading.textures` | first frame that draws them whole, from the opening | 277.00 ms |
| `loading.textures` | CPU of `Renderer::render` while they load, median of 234 | 375.7 µs |
| `loading.textures` | CPU of `Renderer::render` while they load, 90th percentile of 234 | 648.7 µs |
| `loading.textures` | frame while they load, built, drawn and waited for on the GPU, longest of 234 | 23.89 ms |
| `loading.textures` | CPU of `Renderer::render` once they are loaded, median of 5 | 502.6 µs |
| `loading.textures` | CPU of `Renderer::render` once they are loaded, 90th percentile of 5 | 2.13 ms |
| `loading.zone` | cooked zone opened | 63.6 µs |
| `loading.zone` | meshes placed | 73 |
| `loading.zone` | first frame that draws them whole, from the opening | 70.24 ms |
| `loading.zone` | CPU of `Renderer::render` while they load, median of 51 | 747.3 µs |
| `loading.zone` | CPU of `Renderer::render` while they load, 90th percentile of 51 | 1.37 ms |
| `loading.zone` | frame while they load, built, drawn and waited for on the GPU, longest of 51 | 6.32 ms |
| `loading.zone` | CPU of `Renderer::render` once they are loaded, median of 5 | 492.6 µs |
| `loading.zone` | CPU of `Renderer::render` once they are loaded, 90th percentile of 5 | 891.5 µs |

## M6a.2, RX 6900 XT

Commit `b5cb3fa`, on 2026-10-21, measured as the section above. A second run gave, for
`loading.textures`, its meshes whole after 207.77 ms, the longest frame 10.60 ms, the render CPU
at the 90th percentile 557.1 µs while they load; for `loading.zone`, 43.46 ms, 1.32 ms and
753.2 µs. Every frame is within the 16.7 ms of a frame at 60 frames a second
([ADR-0114](adr/0114-hardware-tiers-and-the-gpus-limits.md)), and the render CPU while the meshes
load is near what it is once they are loaded. Loaded without a frame drawn, by a benchmark written
for the occasion and not kept, the zone's meshes came whole after 49.0 and 55.3 ms, where they
took 72.0 and 78.4 ms at `b191845`, and the textures' after 204.1 and 209.2 ms, where they took
223.7 and 236.9 ms. Starting a load, `loading.batch`, takes 0.7 to 1.1 µs on the calling thread,
where it took 11.8 to 18.0 µs at `b191845`; the sample's sky's environment, computed by frame
tasks, 17.7 ms at the median of 11, where it took 19.5 ms on a thread per core
(`measure_the_precomputation` in `crates/slime-render/src/environment.rs`).
[ADR-0118](adr/0118-the-task-pool-and-the-copies-to-the-gpu.md) gives the runs that chose how many
of the loads' tasks run at once and the budget of copies per frame.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `loading.textures` | cooked data opened | 23.6 µs |
| `loading.textures` | meshes placed | 16 |
| `loading.textures` | first frame that draws them whole, from the opening | 215.03 ms |
| `loading.textures` | CPU of `Renderer::render` while they load, median of 342 | 338.2 µs |
| `loading.textures` | CPU of `Renderer::render` while they load, 90th percentile of 342 | 455.2 µs |
| `loading.textures` | frame while they load, built, drawn and waited for on the GPU, longest of 342 | 7.61 ms |
| `loading.textures` | CPU of `Renderer::render` once they are loaded, median of 5 | 342.3 µs |
| `loading.textures` | CPU of `Renderer::render` once they are loaded, 90th percentile of 5 | 706.9 µs |
| `loading.zone` | cooked zone opened | 59.8 µs |
| `loading.zone` | meshes placed | 73 |
| `loading.zone` | first frame that draws them whole, from the opening | 45.85 ms |
| `loading.zone` | CPU of `Renderer::render` while they load, median of 56 | 544.7 µs |
| `loading.zone` | CPU of `Renderer::render` while they load, 90th percentile of 56 | 869.0 µs |
| `loading.zone` | frame while they load, built, drawn and waited for on the GPU, longest of 56 | 4.32 ms |
| `loading.zone` | CPU of `Renderer::render` once they are loaded, median of 5 | 407.3 µs |
| `loading.zone` | CPU of `Renderer::render` once they are loaded, 90th percentile of 5 | 810.6 µs |

## M6a.3, before, RX 6900 XT

Commit `09af9de`, on 2026-10-21: the world before it stamps its changes, with `world.changes.100k`,
whose reader compares every light with a copy, as a reader does without change detection. The
build is not measured in this section, nor in the next. A second run gave `FrameBuilder::build`
11.39 ms at the median, `World::update_transforms` 3.58 ms, the 1 000 lights changed in 10.0 µs and
found by the reader in 3.38 ms.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `world.frame.100k` | `FrameBuilder::build`, median of 20 | 11.43 ms |
| `world.frame.100k` | `FrameBuilder::build`, 90th percentile of 20 | 11.46 ms |
| `world.frame.100k` | instances in a frame | 100000 |
| `world.transforms.100k` | `World::update_transforms` after 1 000 parents moved, median of 20 | 3.57 ms |
| `world.transforms.100k` | `World::update_transforms` after 1 000 parents moved, 90th percentile of 20 | 3.59 ms |
| `world.changes.100k` | 1 000 lights changed through `World::get_mut`, median of 20 | 10.7 µs |
| `world.changes.100k` | 1 000 lights changed through `World::get_mut`, 90th percentile of 20 | 22.5 µs |
| `world.changes.100k` | a reader that compares each light with its copy, the 1 000 found, median of 20 | 3.41 ms |
| `world.changes.100k` | a reader that compares each light with its copy, the 1 000 found, 90th percentile of 20 | 3.72 ms |

## M6a.3, RX 6900 XT

Commit `ad246f0`, on 2026-10-21, measured as the section above. A second run gave
`FrameBuilder::build` 10.03 ms at the median, `World::update_transforms` 3.45 ms, the 1 000 lights
changed in 8.7 µs, found by the reader that compares them with a copy in 3.29 ms and by the one
that asks the world in 17.4 µs, and side by side in 4.1 µs. A frame of 100 000 entities is built
faster: `World::is_hidden` reads a flag where it walked each entity's ancestors.
[ADR-0119](adr/0119-the-worlds-ticks-blocks-and-log.md) gives the runs, on variants of the code,
that chose the size of a block and the width of a tick.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `world.frame.100k` | `FrameBuilder::build`, median of 20 | 10.37 ms |
| `world.frame.100k` | `FrameBuilder::build`, 90th percentile of 20 | 10.60 ms |
| `world.frame.100k` | instances in a frame | 100000 |
| `world.transforms.100k` | `World::update_transforms` after 1 000 parents moved, median of 20 | 3.44 ms |
| `world.transforms.100k` | `World::update_transforms` after 1 000 parents moved, 90th percentile of 20 | 3.46 ms |
| `world.changes.100k` | 1 000 lights changed through `World::get_mut`, median of 40 | 8.6 µs |
| `world.changes.100k` | 1 000 lights changed through `World::get_mut`, 90th percentile of 40 | 9.4 µs |
| `world.changes.100k` | a reader that compares each light with its copy, the 1 000 found, median of 20 | 3.39 ms |
| `world.changes.100k` | a reader that compares each light with its copy, the 1 000 found, 90th percentile of 20 | 3.40 ms |
| `world.changes.100k` | a reader that asks for the lights changed since it last read, the 1 000 found, median of 20 | 18.2 µs |
| `world.changes.100k` | a reader that asks for the lights changed since it last read, the 1 000 found, 90th percentile of 20 | 18.6 µs |
| `world.changes.100k` | the same, the 1 000 side by side, median of 20 | 4.2 µs |
| `world.changes.100k` | the same, the 1 000 side by side, 90th percentile of 20 | 4.2 µs |

## M6a.4, before, RX 6900 XT

Commit `e0bb1c9`, on 2026-10-21: the frame that borrows what it draws, the device already opened
with the adapter's limits, `world.frame.100k` changed as M6a.4 changes it, so that the frame's drop
is counted. The build is not measured in this section, nor in the next. The runs of this section
and the next were made one after the other, before, after, before, after. A second run gave
`FrameBuilder::build` 11.93 ms at the median, and the CPU of `Renderer::render` 20.00, 20.83 and
19.73 ms at 1280 × 800, 1920 × 1080 and 2560 × 1440.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `render.instances.100k` | CPU of `Renderer::render` at 1280 × 800, median of 8 | 19.94 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 8 | 20.04 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1280 × 800, median of 8 | 20.72 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 8 | 20.78 ms |
| `render.instances.100k` | scene pass on the GPU at 1280 × 800, median of what 8 frames reported | 142.8 µs |
| `render.instances.100k` | passes on the GPU, summed at 1280 × 800, median of what 8 frames reported | 265.6 µs |
| `render.instances.100k` | draw calls at 1280 × 800: scene, shadow cascades | 8, 2 |
| `render.instances.100k` | instances that the scene pass drew at 1280 × 800 | 93539 |
| `render.instances.100k` | CPU of `Renderer::render` at 1920 × 1080, median of 8 | 20.19 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 8 | 21.81 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1920 × 1080, median of 8 | 20.93 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 8 | 22.58 ms |
| `render.instances.100k` | scene pass on the GPU at 1920 × 1080, median of what 8 frames reported | 162.8 µs |
| `render.instances.100k` | passes on the GPU, summed at 1920 × 1080, median of what 8 frames reported | 357.8 µs |
| `render.instances.100k` | draw calls at 1920 × 1080: scene, shadow cascades | 8, 2 |
| `render.instances.100k` | instances that the scene pass drew at 1920 × 1080 | 95310 |
| `render.instances.100k` | CPU of `Renderer::render` at 2560 × 1440, median of 8 | 20.45 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 2560 × 1440, 90th percentile of 8 | 20.59 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 2560 × 1440, median of 8 | 21.67 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 2560 × 1440, 90th percentile of 8 | 21.80 ms |
| `render.instances.100k` | scene pass on the GPU at 2560 × 1440, median of what 8 frames reported | 192.2 µs |
| `render.instances.100k` | passes on the GPU, summed at 2560 × 1440, median of what 8 frames reported | 492.1 µs |
| `render.instances.100k` | draw calls at 2560 × 1440: scene, shadow cascades | 8, 2 |
| `render.instances.100k` | instances that the scene pass drew at 2560 × 1440 | 95310 |
| `world.frame.100k` | `FrameBuilder::build`, median of 20 | 11.73 ms |
| `world.frame.100k` | `FrameBuilder::build`, 90th percentile of 20 | 11.80 ms |
| `world.frame.100k` | instances in a frame | 100000 |

## M6a.4, RX 6900 XT

Commit `62cb93b`, on 2026-10-21, measured as the section above. A second run gave
`FrameBuilder::build` 13.28 ms at the median, and the CPU of `Renderer::render` 21.24, 20.19 and
20.12 ms. Building a frame of 100 000 instances, and dropping it, costs 1.3 to 2.7 ms more: each
instance counts a reference to its mesh and one to its material, and gives them back when the frame
is dropped ([ADR-0120](adr/0120-a-frame-that-owns-what-it-draws.md)). Drawing it costs the same,
within the runs' spread; so do the passes on the GPU.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `render.instances.100k` | CPU of `Renderer::render` at 1280 × 800, median of 8 | 19.84 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 8 | 22.44 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1280 × 800, median of 8 | 21.11 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 8 | 22.52 ms |
| `render.instances.100k` | scene pass on the GPU at 1280 × 800, median of what 8 frames reported | 142.6 µs |
| `render.instances.100k` | passes on the GPU, summed at 1280 × 800, median of what 8 frames reported | 265.4 µs |
| `render.instances.100k` | draw calls at 1280 × 800: scene, shadow cascades | 8, 2 |
| `render.instances.100k` | instances that the scene pass drew at 1280 × 800 | 93539 |
| `render.instances.100k` | CPU of `Renderer::render` at 1920 × 1080, median of 8 | 21.83 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 8 | 21.97 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1920 × 1080, median of 8 | 22.63 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 8 | 22.75 ms |
| `render.instances.100k` | scene pass on the GPU at 1920 × 1080, median of what 8 frames reported | 163.0 µs |
| `render.instances.100k` | passes on the GPU, summed at 1920 × 1080, median of what 8 frames reported | 358.0 µs |
| `render.instances.100k` | draw calls at 1920 × 1080: scene, shadow cascades | 8, 2 |
| `render.instances.100k` | instances that the scene pass drew at 1920 × 1080 | 95310 |
| `render.instances.100k` | CPU of `Renderer::render` at 2560 × 1440, median of 8 | 20.31 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 2560 × 1440, 90th percentile of 8 | 21.01 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 2560 × 1440, median of 8 | 21.58 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 2560 × 1440, 90th percentile of 8 | 22.24 ms |
| `render.instances.100k` | scene pass on the GPU at 2560 × 1440, median of what 8 frames reported | 193.1 µs |
| `render.instances.100k` | passes on the GPU, summed at 2560 × 1440, median of what 8 frames reported | 495.4 µs |
| `render.instances.100k` | draw calls at 2560 × 1440: scene, shadow cascades | 8, 2 |
| `render.instances.100k` | instances that the scene pass drew at 2560 × 1440 | 95310 |
| `world.frame.100k` | `FrameBuilder::build`, median of 20 | 14.41 ms |
| `world.frame.100k` | `FrameBuilder::build`, 90th percentile of 20 | 14.61 ms |
| `world.frame.100k` | instances in a frame | 100000 |

## M6a.7b, before, RX 6900 XT

Commit `09adade`, on 2026-10-21: the save that compares the world with a start world that the game
makes again, with `world.save.100k`. The build is not measured in this section, nor in the next. A
second run gave the save 655.03 ms at the median, its load 8.62 ms, its size 62 kB.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `world.save.100k` | save of 100000 entities, 1000 moved, the level spawned again to compare with, as a game makes its start, median of 5 | 690.68 ms |
| `world.save.100k` | its load, median of 5 | 9.33 ms |
| `world.save.100k` | its size | 62 kB |

## M6a.7b, RX 6900 XT

Commit `7dffb22`, on 2026-10-21, measured as the section above: the level settled, the save
without a start world ([ADR-0125](adr/0125-saves-per-scene.md)). A second run gave the save
7.08 ms at the median, its load 9.80 ms. The save costs what changed: the values changed, read
from the copies that the world kept of them, and one walk of the world's entities without their
components; its size is the same, and so is its load, which applies what changed.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `world.save.100k` | save of 100000 entities, 1000 moved, median of 5 | 6.82 ms |
| `world.save.100k` | its load, median of 5 | 9.77 ms |
| `world.save.100k` | its size | 62 kB |

## M6a.8a, RX 6900 XT

Commit `1dbe75c`, on 2026-10-22: views apart from the renderer
([ADR-0126](adr/0126-views-apart-from-the-renderer.md)). The GPU memory is what wgpu's allocator
counts (`Gpu::memory_in_use`), without what the driver keeps for itself, printed by the test
`four_views_take_one_cascades_atlas_where_four_renderers_took_four` of `slime-render`'s
`tests/views.rs`: four views of 1280 × 720 drawing a floor and two cubes under a sun that casts
shadows, with the bloom, with one renderer, against four renderers drawing a view each; the
outputs, created before, are not counted. On lavapipe the same test gave 279.7 MiB and 471.7 MiB.
The benchmark `render.instances.100k`, which draws through one view, costs what it cost at M6a.4,
in its section above; the build is not measured.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a debug build of the
test and a release build of the benchmark.

| Benchmark or test | Measure | Value |
|---|---|---|
| `tests/views.rs` | GPU memory of four views of 1280 × 720 with the sun, one renderer | 296.4 MiB |
| `tests/views.rs` | GPU memory of four renderers with a view each | 491.9 MiB |
| `render.instances.100k` | CPU of `Renderer::render` at 1280 × 800, median of 8 | 19.33 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 8 | 19.57 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1280 × 800, median of 8 | 20.25 ms |
| `render.instances.100k` | passes on the GPU, summed at 1280 × 800, median of what 8 frames reported | 264.7 µs |
| `render.instances.100k` | CPU of `Renderer::render` at 1920 × 1080, median of 8 | 19.60 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1920 × 1080, median of 8 | 20.61 ms |
| `render.instances.100k` | passes on the GPU, summed at 1920 × 1080, median of what 8 frames reported | 355.0 µs |
| `render.instances.100k` | CPU of `Renderer::render` at 2560 × 1440, median of 8 | 20.91 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 2560 × 1440, median of 8 | 21.88 ms |
| `render.instances.100k` | passes on the GPU, summed at 2560 × 1440, median of what 8 frames reported | 493.3 µs |

## M6b.1a, RX 6900 XT

Commit `643302a`, on 2026-10-22: the world's commands, which give back what undoes them
([ADR-0134](adr/0134-the-editors-core.md)). The benchmark `world.undo.1k` is new: a subtree of
1 000 entities, each with a name, a transform and a light, in a level of 100 000, deleted by the
world's command, then restored by the command that deleting gave back, twenty times. The audit of
0.4 estimated 10 to 15 ms for the two ([ADR-0109](adr/0109-stable-references-and-undo.md)). The
build is not measured.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `world.undo.1k` | 1000 entities deleted, in a level of 100000, median and 90th percentile of 20 | 1.37 ms / 1.82 ms |
| `world.undo.1k` | restored, median and 90th percentile of 20 | 1.83 ms / 2.15 ms |
| `world.undo.1k` | deleted then restored, median and 90th percentile of 20 | 3.24 ms / 4.00 ms |

## M6b.2, RX 6900 XT

Commit `97a0815`, on 2026-10-23: the interface's text ([ADR-0136](adr/0136-the-interfaces-text.md)).
The benchmarks `ui.text.labels.1k`, `ui.text.paragraphs` and `ui.text.raster` are new, and measure
what the study of the UI framework measured on this machine
([ADR-0132](adr/0132-the-engines-ui-framework.md)), with the fonts of slime-ui's tests named in
their stacks where the study let fontconfig's fallback find most of them. The milestone asks that
they stay within 20 % of the study's figures. They do, or are under them: 1 000 labels take 7.45 ms
against the study's 7.0 ms; every paragraph that the study measured takes less than it did, the
study counting the system's fallback, Arabic most of all; glyphs rasterize within the study's
figures. One figure is above: a paragraph's lines broken again in another width, 71.4 µs against the
study's 20 to 45 µs, because `TextLayout::set_bounds` also gathers the glyphs again for drawing,
which the study did not. Timed once inside it, in a build instrumented for that and not kept,
breaking the lines took 21 µs, aligning them 70 ns and gathering the glyphs 48 µs. The build is not
measured.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `ui.text.labels.1k` | 1 000 labels laid out, median of 15 | 7.45 ms |
| `ui.text.labels.1k` | 1 000 labels laid out, 90th percentile of 15 | 7.53 ms |
| `ui.text.paragraphs` | a paragraph of 2123 characters of english laid out, median of 15 | 357.9 µs |
| `ui.text.paragraphs` | a paragraph of 2025 characters of arabic laid out, median of 15 | 810.0 µs |
| `ui.text.paragraphs` | a paragraph of 2102 characters of hindi laid out, median of 15 | 868.9 µs |
| `ui.text.paragraphs` | a paragraph of 2028 characters of thai laid out, median of 15 | 576.6 µs |
| `ui.text.paragraphs` | a paragraph of 2026 characters of japanese laid out, median of 15 | 1.16 ms |
| `ui.text.paragraphs` | a paragraph of 2003 characters of chinese laid out, median of 15 | 490.1 µs |
| `ui.text.paragraphs` | a paragraph of 2069 characters of emoji laid out, median of 15 | 621.9 µs |
| `ui.text.paragraphs` | the English paragraph's lines broken again in another width, median of 15 | 71.4 µs |
| `ui.text.raster` | a Latin letter rasterized at 14 pixels, median of 15 | 1.2 µs |
| `ui.text.raster` | a Latin letter rasterized at 32 pixels, median of 15 | 1.9 µs |
| `ui.text.raster` | a Latin letter rasterized at 64 pixels, median of 15 | 3.1 µs |
| `ui.text.raster` | a Chinese character rasterized at 14 pixels, median of 15 | 4.0 µs |
| `ui.text.raster` | a Chinese character rasterized at 32 pixels, median of 15 | 6.7 µs |
| `ui.text.raster` | a Chinese character rasterized at 64 pixels, median of 15 | 10.9 µs |
| `ui.text.raster` | an emoji rasterized at 14 pixels, median of 15 | 201.6 µs |
| `ui.text.raster` | an emoji rasterized at 32 pixels, median of 15 | 262.9 µs |
| `ui.text.raster` | an emoji rasterized at 64 pixels, median of 15 | 377.2 µs |

The memory that a laid-out text holds, which `TextLayout::memory` estimates for `TextCache`'s
budget, was measured with an allocator that counts the bytes it hands out, in a program outside the
repository, since the engine's code holds no `unsafe`: 100 layouts kept of each text, of 20
characters and of some 2 000, laid out 400 pixels wide, none cut. The estimate is fitted to these
figures: 3 KiB, 50 bytes per byte of text, 16 per glyph and some 400 per run.

| Text | Bytes of text | Glyphs | Runs | Measured per layout | Estimated | Estimate against measure |
|---|---|---|---|---|---|---|
| English, 20 characters | 20 | 20 | 1 | 5 000 B | 4 830 B | −3 % |
| English, 2 123 characters | 2 127 | 2 119 | 37 | 150 291 B | 159 532 B | +6 % |
| Arabic, 20 characters | 37 | 26 | 1 | 5 840 B | 5 776 B | −1 % |
| Arabic, 2 025 characters | 3 443 | 2 534 | 156 | 302 834 B | 280 126 B | −7 % |
| Hindi, 20 characters | 54 | 20 | 1 | 6 677 B | 6 530 B | −2 % |
| Hindi, 2 102 characters | 5 396 | 1 916 | 28 | 315 475 B | 315 792 B | 0 % |
| Thai, 20 characters | 60 | 21 | 1 | 6 965 B | 6 846 B | −2 % |
| Thai, 2 028 characters | 6 034 | 2 044 | 33 | 345 195 B | 351 930 B | +2 % |
| Japanese, 20 characters | 60 | 20 | 7 | 8 359 B | 9 266 B | +11 % |
| Japanese, 2 026 characters | 6 078 | 2 026 | 894 | 710 761 B | 704 688 B | −1 % |
| Chinese, 20 characters | 60 | 20 | 1 | 6 970 B | 6 830 B | −2 % |
| Chinese, 2 003 characters | 6 009 | 2 003 | 72 | 355 495 B | 367 106 B | +3 % |
| English with emoji, 20 characters | 22 | 20 | 3 | 5 201 B | 5 742 B | +10 % |
| English with emoji, 2 069 characters | 2 873 | 1 913 | 349 | 362 388 B | 320 208 B | −12 % |

## M6b.3a, RX 6900 XT

Commit `6fc0c78`, on 2026-10-23: the interface's canvas ([ADR-0137](adr/0137-the-canvas.md)), and
the high tier at 3840 × 2160 ([ADR-0138](adr/0138-the-high-tier-at-3840-2160.md)). The render
benchmarks measure the high tier at 3840 × 2160, where the sections before measured 2560 × 1440;
`render.canvas` is new. ADR-0137 bounds the interface's GPU time on each tier: the game's menu holds
it at every tier, and the editor's screen at the modest and high tiers, with room to spare. Writing
the canvas's items costs the CPU about a third of a microsecond each. The machine was otherwise
idle. The build is not measured.

How the canvas came to its figures, measured during the milestone on the working tree, each pair of
runs one right after the other, the versions before not kept. The GPU also draws the owner's
desktop, whose work moved the figures by a fifth over the afternoon: only the figures of one row
compare. The editor's screen, its colors mixed linearly unless a row says otherwise:

| Change | Measure | Before | After |
|---|---|---|---|
| The item read again by the fragment shader, rather than handed on as values of the vertices | GPU at 1280 × 800 | 101.2 µs | 76.3 µs |
| An experiment, not kept: the fragment shader returning the item's color alone, what writing the canvas's pixels costs | GPU at 1280 × 800 | 76.3 µs | 40.7 µs |
| Raph Levien's shadows and the items cut to their clip, rather than shadows integrated over four rows | GPU at 1280 × 800, 1920 × 1080 and 2560 × 1440 | 62.7, 110.3 and 192.2 µs | 44.3, 75.7 and 128.9 µs |
| Colors mixed as sRGB values, encoded once per item on the CPU, against not encoded, in turn three times | GPU at 1280 × 800 | 53.9, 53.2 and 53.0 µs | 50.3, 53.4 and 54.0 µs |

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `render.instances.10k` | CPU of `Renderer::render` at 1280 × 800, median of 30 | 2.28 ms |
| `render.instances.10k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 30 | 3.04 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1280 × 800, median of 30 | 2.65 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 30 | 3.59 ms |
| `render.instances.10k` | scene pass on the GPU at 1280 × 800, median of what 30 frames reported | 39.8 µs |
| `render.instances.10k` | passes on the GPU, summed at 1280 × 800, median of what 30 frames reported | 151.6 µs |
| `render.instances.10k` | draw calls at 1280 × 800: scene, shadow cascades | 8, 3 |
| `render.instances.10k` | instances that the scene pass drew at 1280 × 800 | 9343 |
| `render.instances.10k` | CPU of `Renderer::render` at 1920 × 1080, median of 30 | 2.23 ms |
| `render.instances.10k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 30 | 2.54 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1920 × 1080, median of 30 | 2.62 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 30 | 2.82 ms |
| `render.instances.10k` | scene pass on the GPU at 1920 × 1080, median of what 30 frames reported | 53.2 µs |
| `render.instances.10k` | passes on the GPU, summed at 1920 × 1080, median of what 30 frames reported | 220.5 µs |
| `render.instances.10k` | draw calls at 1920 × 1080: scene, shadow cascades | 8, 3 |
| `render.instances.10k` | instances that the scene pass drew at 1920 × 1080 | 9517 |
| `render.instances.10k` | CPU of `Renderer::render` at 3840 × 2160, median of 30 | 2.40 ms |
| `render.instances.10k` | CPU of `Renderer::render` at 3840 × 2160, 90th percentile of 30 | 2.57 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 3840 × 2160, median of 30 | 3.23 ms |
| `render.instances.10k` | frame, until the GPU finished it, at 3840 × 2160, 90th percentile of 30 | 3.44 ms |
| `render.instances.10k` | scene pass on the GPU at 3840 × 2160, median of what 30 frames reported | 129.3 µs |
| `render.instances.10k` | passes on the GPU, summed at 3840 × 2160, median of what 30 frames reported | 637.0 µs |
| `render.instances.10k` | draw calls at 3840 × 2160: scene, shadow cascades | 8, 3 |
| `render.instances.10k` | instances that the scene pass drew at 3840 × 2160 | 9517 |
| `render.unseen.10k` | CPU of `Renderer::render` at 1280 × 800, median of 30 | 1.65 ms |
| `render.unseen.10k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 30 | 2.12 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1280 × 800, median of 30 | 1.80 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 30 | 2.33 ms |
| `render.unseen.10k` | scene pass on the GPU at 1280 × 800, median of what 30 frames reported | 2.3 µs |
| `render.unseen.10k` | passes on the GPU, summed at 1280 × 800, median of what 30 frames reported | 113.5 µs |
| `render.unseen.10k` | draw calls at 1280 × 800: scene, shadow cascades | 0, 4 |
| `render.unseen.10k` | instances that the scene pass drew at 1280 × 800 | 0 |
| `render.unseen.10k` | CPU of `Renderer::render` at 1920 × 1080, median of 30 | 1.34 ms |
| `render.unseen.10k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 30 | 1.40 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1920 × 1080, median of 30 | 1.51 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 30 | 1.58 ms |
| `render.unseen.10k` | scene pass on the GPU at 1920 × 1080, median of what 30 frames reported | 2.4 µs |
| `render.unseen.10k` | passes on the GPU, summed at 1920 × 1080, median of what 30 frames reported | 168.3 µs |
| `render.unseen.10k` | draw calls at 1920 × 1080: scene, shadow cascades | 0, 4 |
| `render.unseen.10k` | instances that the scene pass drew at 1920 × 1080 | 0 |
| `render.unseen.10k` | CPU of `Renderer::render` at 3840 × 2160, median of 30 | 1.42 ms |
| `render.unseen.10k` | CPU of `Renderer::render` at 3840 × 2160, 90th percentile of 30 | 1.48 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 3840 × 2160, median of 30 | 2.06 ms |
| `render.unseen.10k` | frame, until the GPU finished it, at 3840 × 2160, 90th percentile of 30 | 2.12 ms |
| `render.unseen.10k` | scene pass on the GPU at 3840 × 2160, median of what 30 frames reported | 2.8 µs |
| `render.unseen.10k` | passes on the GPU, summed at 3840 × 2160, median of what 30 frames reported | 480.3 µs |
| `render.unseen.10k` | draw calls at 3840 × 2160: scene, shadow cascades | 0, 4 |
| `render.unseen.10k` | instances that the scene pass drew at 3840 × 2160 | 0 |
| `render.instances.100k` | CPU of `Renderer::render` at 1280 × 800, median of 8 | 19.14 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 1280 × 800, 90th percentile of 8 | 19.60 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1280 × 800, median of 8 | 19.85 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1280 × 800, 90th percentile of 8 | 20.79 ms |
| `render.instances.100k` | scene pass on the GPU at 1280 × 800, median of what 8 frames reported | 146.3 µs |
| `render.instances.100k` | passes on the GPU, summed at 1280 × 800, median of what 8 frames reported | 268.3 µs |
| `render.instances.100k` | draw calls at 1280 × 800: scene, shadow cascades | 8, 2 |
| `render.instances.100k` | instances that the scene pass drew at 1280 × 800 | 93539 |
| `render.instances.100k` | CPU of `Renderer::render` at 1920 × 1080, median of 8 | 21.34 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 1920 × 1080, 90th percentile of 8 | 21.60 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1920 × 1080, median of 8 | 22.09 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 1920 × 1080, 90th percentile of 8 | 22.35 ms |
| `render.instances.100k` | scene pass on the GPU at 1920 × 1080, median of what 8 frames reported | 167.9 µs |
| `render.instances.100k` | passes on the GPU, summed at 1920 × 1080, median of what 8 frames reported | 359.3 µs |
| `render.instances.100k` | draw calls at 1920 × 1080: scene, shadow cascades | 8, 2 |
| `render.instances.100k` | instances that the scene pass drew at 1920 × 1080 | 95310 |
| `render.instances.100k` | CPU of `Renderer::render` at 3840 × 2160, median of 8 | 21.42 ms |
| `render.instances.100k` | CPU of `Renderer::render` at 3840 × 2160, 90th percentile of 8 | 21.72 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 3840 × 2160, median of 8 | 23.30 ms |
| `render.instances.100k` | frame, until the GPU finished it, at 3840 × 2160, 90th percentile of 8 | 23.72 ms |
| `render.instances.100k` | scene pass on the GPU at 3840 × 2160, median of what 8 frames reported | 289.5 µs |
| `render.instances.100k` | passes on the GPU, summed at 3840 × 2160, median of what 8 frames reported | 941.7 µs |
| `render.instances.100k` | draw calls at 3840 × 2160: scene, shadow cascades | 8, 2 |
| `render.instances.100k` | instances that the scene pass drew at 3840 × 2160 | 95310 |
| `render.shaders` | pipelines made again, shaders unchanged, median of 5 | 11.92 ms |
| `render.shaders` | pipelines made again after an edit of the scene's shader, median of 5 | 11.67 ms |
| `render.canvas` | CPU of `Renderer::draw_canvas`, the editor's screen at 1280 × 800, median of 30 | 659.6 µs |
| `render.canvas` | CPU of `Renderer::draw_canvas`, the editor's screen at 1280 × 800, 90th percentile of 30 | 704.1 µs |
| `render.canvas` | canvas on the GPU, the editor's screen at 1280 × 800, median of what 30 frames reported | 44.5 µs |
| `render.canvas` | items and draws, the editor's screen at 1280 × 800 | 19475, 17 |
| `render.canvas` | CPU of `Renderer::draw_canvas`, the game's menu at 1280 × 800, median of 30 | 207.5 µs |
| `render.canvas` | CPU of `Renderer::draw_canvas`, the game's menu at 1280 × 800, 90th percentile of 30 | 214.3 µs |
| `render.canvas` | canvas on the GPU, the game's menu at 1280 × 800, median of what 30 frames reported | 30.7 µs |
| `render.canvas` | items and draws, the game's menu at 1280 × 800 | 4155, 3 |
| `render.canvas` | CPU of `Renderer::draw_canvas`, the editor's screen at 1920 × 1080, median of 30 | 646.8 µs |
| `render.canvas` | CPU of `Renderer::draw_canvas`, the editor's screen at 1920 × 1080, 90th percentile of 30 | 654.9 µs |
| `render.canvas` | canvas on the GPU, the editor's screen at 1920 × 1080, median of what 30 frames reported | 76.0 µs |
| `render.canvas` | items and draws, the editor's screen at 1920 × 1080 | 19377, 17 |
| `render.canvas` | CPU of `Renderer::draw_canvas`, the game's menu at 1920 × 1080, median of 30 | 198.8 µs |
| `render.canvas` | CPU of `Renderer::draw_canvas`, the game's menu at 1920 × 1080, 90th percentile of 30 | 203.8 µs |
| `render.canvas` | canvas on the GPU, the game's menu at 1920 × 1080, median of what 30 frames reported | 60.1 µs |
| `render.canvas` | items and draws, the game's menu at 1920 × 1080 | 4155, 3 |
| `render.canvas` | CPU of `Renderer::draw_canvas`, the editor's screen at 3840 × 2160, median of 30 | 647.5 µs |
| `render.canvas` | CPU of `Renderer::draw_canvas`, the editor's screen at 3840 × 2160, 90th percentile of 30 | 664.3 µs |
| `render.canvas` | canvas on the GPU, the editor's screen at 3840 × 2160, median of what 30 frames reported | 274.7 µs |
| `render.canvas` | items and draws, the editor's screen at 3840 × 2160 | 19377, 17 |
| `render.canvas` | CPU of `Renderer::draw_canvas`, the game's menu at 3840 × 2160, median of 30 | 204.0 µs |
| `render.canvas` | CPU of `Renderer::draw_canvas`, the game's menu at 3840 × 2160, 90th percentile of 30 | 214.1 µs |
| `render.canvas` | canvas on the GPU, the game's menu at 3840 × 2160, median of what 30 frames reported | 223.7 µs |
| `render.canvas` | items and draws, the game's menu at 3840 × 2160 | 4155, 3 |

## M6b.3b, RX 6900 XT

Commit `30fcdad`, on 2026-10-24: the interface's text drawn from a display list through
`slime-ui-render` ([ADR-0139](adr/0139-text-drawn-and-the-display-list.md)). The benchmark
`ui.draw.menu` is new: a game's menu of real text, some 2 400 glyphs, drawn through the bridge at
the hardware tiers' resolutions. Its canvas holds the interface's budget at every tier, as
[ADR-0137](adr/0137-the-canvas.md) sets it; its first draw, which rasterizes some 250 glyphs into
the atlas, takes a little over 2 ms, then the bridge takes some 0.36 ms of the CPU a frame. The
machine was otherwise idle. The build is not measured.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `ui.draw.menu` | the first draw at 1280 × 800, its 254 glyphs rasterized into the atlas | 2.35 ms |
| `ui.draw.menu` | CPU of `UiRenderer::draw` at 1280 × 800, median of 30 | 362.5 µs |
| `ui.draw.menu` | CPU of `UiRenderer::draw` at 1280 × 800, 90th percentile of 30 | 370.5 µs |
| `ui.draw.menu` | canvas on the GPU at 1280 × 800, median of what 30 frames reported | 27.7 µs |
| `ui.draw.menu` | glyphs drawn, items, and the atlas's memory at 1280 × 800 | 2440, 2645, 1 MiB |
| `ui.draw.menu` | the first draw at 1920 × 1080, its 258 glyphs rasterized into the atlas | 2.10 ms |
| `ui.draw.menu` | CPU of `UiRenderer::draw` at 1920 × 1080, median of 30 | 359.2 µs |
| `ui.draw.menu` | CPU of `UiRenderer::draw` at 1920 × 1080, 90th percentile of 30 | 366.6 µs |
| `ui.draw.menu` | canvas on the GPU at 1920 × 1080, median of what 30 frames reported | 53.6 µs |
| `ui.draw.menu` | glyphs drawn, items, and the atlas's memory at 1920 × 1080 | 2440, 2645, 1 MiB |
| `ui.draw.menu` | the first draw at 3840 × 2160, its 256 glyphs rasterized into the atlas | 2.22 ms |
| `ui.draw.menu` | CPU of `UiRenderer::draw` at 3840 × 2160, median of 30 | 358.9 µs |
| `ui.draw.menu` | CPU of `UiRenderer::draw` at 3840 × 2160, 90th percentile of 30 | 366.2 µs |
| `ui.draw.menu` | canvas on the GPU at 3840 × 2160, median of what 30 frames reported | 199.8 µs |
| `ui.draw.menu` | glyphs drawn, items, and the atlas's memory at 3840 × 2160 | 2440, 2645, 1 MiB |

## M6b.4, RX 6900 XT

The interface's tree of elements, before its style sheets and themes
([ADR-0140](adr/0140-the-tree-of-elements.md)), on 2026-10-24. The benchmarks `ui.tree.editor` and
`ui.tree.list.100k` are new; they use the CPU alone, an AMD Ryzen 7 5800X. A frame of the list of
100 000 rows holds the budget that ADR-0140 sets, 0.5 ms at its 90th percentile, scrolled or
standing still: the frames that build new rows, one in three, are those of that percentile. A tree
of the editor's size takes 2.4 ms after one value of its inspector changed, its panel built again,
and 0.28 ms with nothing changed, which gathers the display list from what its elements drew before.
The machine was otherwise idle. The build is not measured.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `ui.tree.editor` | elements | 12487 |
| `ui.tree.editor` | items of the display list | 9180 |
| `ui.tree.editor` | the first frame: built, its texts and its tree laid out, drawn, median of 15 | 133.98 ms |
| `ui.tree.editor` | the first frame: built, its texts and its tree laid out, drawn, 90th percentile of 15 | 143.64 ms |
| `ui.tree.editor` | a frame after one value of the inspector changed, median of 15 | 2.36 ms |
| `ui.tree.editor` | a frame after one value of the inspector changed, 90th percentile of 15 | 2.40 ms |
| `ui.tree.editor` | a frame with nothing changed, median of 15 | 280.0 µs |
| `ui.tree.editor` | a frame with nothing changed, 90th percentile of 15 | 303.8 µs |
| `ui.tree.list.100k` | elements | 164 |
| `ui.tree.list.100k` | a frame scrolled by 7 pixels, median of 15 | 58.8 µs |
| `ui.tree.list.100k` | a frame scrolled by 7 pixels, 90th percentile of 15 | 142.9 µs |
| `ui.tree.list.100k` | a frame standing still, median of 15 | 3.1 µs |
| `ui.tree.list.100k` | a frame standing still, 90th percentile of 15 | 3.2 µs |
