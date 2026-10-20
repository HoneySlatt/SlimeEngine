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
