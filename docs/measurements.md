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
read. The figures that the known debt ([`debt.md`](debt.md)) and the ADRs give were measured once; this file holds
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
on the CPU at each frame, which the known debt in [`debt.md`](debt.md) says. `FrameBuilder::build` over
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

## M6b.4a, RX 6900 XT

The interface's tree of elements, before its style sheets and themes
([ADR-0140](adr/0140-the-tree-of-elements.md)), on 2026-10-24. The benchmarks `ui.tree.editor` and
`ui.tree.list.100k` are new; they use the CPU alone, an AMD Ryzen 7 5800X. A frame of the list of
100 000 rows holds the budget that ADR-0140 sets, 0.25 ms at its 90th percentile, scrolled or
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

## M6b.4b, RX 6900 XT

Style sheets and themes ([ADR-0141](adr/0141-style-sheets-and-themes.md)), on 2026-10-25, on the
CPU alone, an AMD Ryzen 7 5800X. `ui.tree.editor` now styles its tree with a style sheet and a
theme, and measures a frame after the theme changed, which styles every element again, draws it
again, and gathers the display list: 16 ms. Made each time through reflection, the styles took
228 ms in that frame, and the first frame 358 ms; kept for the rules that select an element and its
own style, and shared, they take what this table says, the first frame as long as before styles
came. The buttons and panels that the style sheet fills add 1 533 items to the display list. The
machine was otherwise idle. The table was taken before a read-only review's fixes, which add a
comparison to each element styled again; taken again after them, while another program held three
quarters of a core, its frames took 4 to 17 % longer. The build is not measured.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `ui.tree.editor` | elements | 12487 |
| `ui.tree.editor` | items of the display list | 10713 |
| `ui.tree.editor` | the first frame: built, its texts and its tree laid out, drawn, median of 15 | 131.38 ms |
| `ui.tree.editor` | the first frame: built, its texts and its tree laid out, drawn, 90th percentile of 15 | 141.03 ms |
| `ui.tree.editor` | a frame after one value of the inspector changed, median of 15 | 2.45 ms |
| `ui.tree.editor` | a frame after one value of the inspector changed, 90th percentile of 15 | 2.48 ms |
| `ui.tree.editor` | a frame with nothing changed, median of 15 | 291.7 µs |
| `ui.tree.editor` | a frame with nothing changed, 90th percentile of 15 | 298.2 µs |
| `ui.tree.editor` | a frame after the theme changed, every element styled again, median of 15 | 15.70 ms |
| `ui.tree.editor` | a frame after the theme changed, every element styled again, 90th percentile of 15 | 15.95 ms |
| `ui.tree.list.100k` | elements | 164 |
| `ui.tree.list.100k` | a frame scrolled by 7 pixels, median of 15 | 55.3 µs |
| `ui.tree.list.100k` | a frame scrolled by 7 pixels, 90th percentile of 15 | 140.8 µs |
| `ui.tree.list.100k` | a frame standing still, median of 15 | 3.1 µs |
| `ui.tree.list.100k` | a frame standing still, 90th percentile of 15 | 3.2 µs |

## M6b.8, RX 6900 XT

The dev overlay's world panel on the engine's interface
([ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)), on 2026-10-27, on the CPU
alone, an AMD Ryzen 7 5800X, before the programs move onto it. egui's world panel cost 125 to
145 ms a frame over 100 000 entities (G46); this one, standing still, scrolled, or after the entity
selected changed, costs what one over a few entities does: its list builds only the rows that show,
and it builds the entity selected again only once it changed. A frame after an entity was renamed,
or spawned or despawned, makes the list of entities again, every name read and filtered: 7 ms at
100 000 entities, as egui's panel did too. The machine was not quite idle: `ui.tree.list.100k`,
run beside it, took 1.2 times its figure of M6b.4b while scrolled, and 1.5 times standing still.
The build is not measured.

16 logical CPUs, linux on x86_64, GPU `AMD Radeon RX 6900 XT (RADV NAVI21)`, a release build.

| Benchmark | Measure | Value |
|---|---|---|
| `ui.tree.list.100k` | elements | 164 |
| `ui.tree.list.100k` | a frame scrolled by 7 pixels, median of 15 | 66.2 µs |
| `ui.tree.list.100k` | a frame scrolled by 7 pixels, 90th percentile of 15 | 182.7 µs |
| `ui.tree.list.100k` | a frame standing still, median of 15 | 4.5 µs |
| `ui.tree.list.100k` | a frame standing still, 90th percentile of 15 | 4.6 µs |
| `devui.world_panel.100k` | elements | 61 |
| `devui.world_panel.100k` | a frame standing still, median of 15 | 4.3 µs |
| `devui.world_panel.100k` | a frame standing still, 90th percentile of 15 | 5.3 µs |
| `devui.world_panel.100k` | a frame whose world changed, median of 15 | 98.0 µs |
| `devui.world_panel.100k` | a frame whose world changed, 90th percentile of 15 | 113.7 µs |
| `devui.world_panel.100k` | a frame scrolled by a row, median of 15 | 29.8 µs |
| `devui.world_panel.100k` | a frame scrolled by a row, 90th percentile of 15 | 35.3 µs |
| `devui.world_panel.100k` | a frame after an entity was renamed, median of 15 | 6.93 ms |
| `devui.world_panel.100k` | a frame after an entity was renamed, 90th percentile of 15 | 7.62 ms |

## M6b.8b, Ryzen 7 5800X

Commit `616e799`, on 2026-10-27: what *Hunna Wood*'s development build adds to its build for
shipping, now that the dev overlay is in the first alone
([ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)), a release build on an AMD Ryzen
7 5800X, 16 logical CPUs, after the build for shipping, its dependencies built. Building the game
alone again takes 6.0 s; the development build, after the build for shipping, takes 17.0 s, as it
builds the development channel, the overlay, the canvas's bridge, the editor's look and the crates
whose features change, and the game. The overlay on egui was estimated to cost about 10 s of the
game's release build, which the build for shipping paid then. The benchmarks are not measured.

| Build | Measure | Value |
|---|---|---|
| `cargo build --release -p hunnawood --no-default-features` | the game alone built again | 6.0 s |
| `cargo build --release -p hunnawood` | after the build for shipping | 17.0 s |

## M6c.0, Ryzen 7 5800X

Commits `b5adc5a` and `9771c5c`, on 2026-10-27: the development channel's round trip, and the
agent's session that drives *Hunna Wood* in the checks, on an AMD Ryzen 7 5800X with lavapipe. Each
message of the channel went out in four writes, on sockets that held a write until the one before was
acknowledged, which the receiver delays: a request waited for that delay whatever the game's frame
rate. Written in one write, but for a payload, which follows in a second, and with Nagle's
algorithm off at both ends, it waits for nothing; no delayed acknowledgement remains in the round
trip. The round trip is the median of 21 requests that a
thread answers at once, on the loopback, as `slime-channel`'s test
`a_request_is_answered_in_well_under_the_delay_of_an_acknowledgement` measures it, the fix undone
for the first figure.

The session (`slime-mcp`'s test `drive`) waits for a frame of the game at each request, and a frame
of the zone with its shadows took about 13 s on four of the machine's cores (`taskset -c 0-3`, as
CI's runners have four), about 25 s on CI's runners, where the session reached its limit of 15
minutes. It now draws the view without shadows, without anti-aliasing and at half its size, set
through the MCP server once the game is paused; the game's own runs in the checks still draw the
zone whole. The benchmarks are not measured.

| Measure | Before | After |
|---|---|---|
| a request of the development channel, answered at once, median of 21 | 82 ms | 0.14 ms |
| the session `drive`, on four cores, lavapipe | 426 s | 21 s |

## M6c.2a, RX 6900 XT

On 2026-10-28, before the commits of M6c.2a: what `PASSES_PER_SUBMISSION`
(`crates/slime-render/src/graph.rs`) and the bloom cost the CPU of `Renderer::render` while the
desktop fills the GPU's memory that the CPU maps, where an editor's viewport draws
([ADR-0155](adr/0155-the-editors-design.md), [ADR-0158](adr/0158-the-viewport.md)). On an AMD Ryzen
7 5800X and an RX 6900 XT through RADV (Mesa 26.2.4), whose memory that the CPU maps is a window of
256 MiB without resizable BAR, which the owner's desktop and its programs held whole at each run
(`mem_info_vis_vram_used` at 268 435 456 bytes before each). A release build, the benchmark
`render.instances.10k` (`benches/README.md`), its scene's bloom at its strength of 0.04 or at 0,
`PASSES_PER_SUBMISSION` changed for each run and given back after. Medians of 30 frames, in
milliseconds of CPU, at 1280 × 800, 1920 × 1080 and 3840 × 2160; the GPU's passes, summed, took
0.18, 0.26 and 0.69 ms whatever the passes per submission. A second run of 4, 8 and 16 with the
bloom gave the 90th percentiles of the last table, and medians within 0.2 ms of the first's.

| Passes per submission | Bloom | 1280 × 800 | 1920 × 1080 | 3840 × 2160 |
|---|---|---|---|---|
| 1 | on | 8.26 | 8.88 | 9.72 |
| 2 | on | 5.30 | 6.13 | 6.64 |
| 4, today's | on | 4.23 | 4.68 | 5.30 |
| 8 | on | 3.77 | 4.43 | 4.82 |
| 16, a whole frame | on | 3.51 | 4.12 | 4.65 |
| 1 | off | 3.67 | 3.62 | 3.57 |
| 2 | off | 2.94 | 2.97 | 2.97 |
| 4, today's | off | 2.92 | 2.96 | 2.87 |
| 8 | off | 2.67 | 2.61 | 2.59 |
| 16, a whole frame | off | 2.63 | 2.53 | 2.51 |

| Passes per submission, bloom on | 1280 × 800, 90th percentile | 1920 × 1080 | 3840 × 2160 |
|---|---|---|---|
| 4, today's | 4.59 | 5.18 | 5.44 |
| 8 | 3.90 | 4.52 | 5.05 |
| 16, a whole frame | 3.68 | 4.32 | 4.91 |

What it says: in the stall, a frame's CPU grows with its submissions, the bloom's many small passes
the most of them; a whole frame in one submission costs least, and its 90th percentile shows none of
the occasional 8 ms that [ADR-0041](adr/0041-render-graph.md) measured for it at M3b.8, so that the
reason to keep 4 holds no longer on this machine. The owner chose 16 on 2026-10-28
([ADR-0158](adr/0158-the-viewport.md)), which the benchmarks measure from M6c.2a's commits on. The
viewport draws only when what it shows changes, which hides most of it: flying draws it at every
frame, at some 4 to 5 ms of CPU here.

## M6c.4d, RX 6900 XT

On 2026-10-28, from the commits of M6c.4d: Play's time to control, from the request to play to the
first state of the game whose time it no longer holds, which Play measures and the editor's log
says, as `cargo run [--release] -p slime-benches -- play.zone play.level.100k` reads it from an
editor run without its window (`benches/README.md`): five runs after a first, which builds the game
and fills the system's caches. Of each time, the editor took a part writing the level, before Play's
thread starts, and cargo a part finding nothing to build. *Hunna Wood*'s zone is played in a window
on the owner's compositor, drawn by the RX 6900 XT; the level of 100 000 entities by the `world`
example, without a window. Release builds the editor and the game in release, debug in the default
profile. Two runs of the benchmarks, on the owner's desktop, gave medians apart by up to 0.4 s on
the level of 100 000 entities in release and 1.4 s in debug; the table holds the last of each.

| Measure | Release | Debug |
|---|---|---|
| Play on the zone, to control, median of 5 | 0.60 s | 1.15 s |
| Play on the zone, to control, 90th percentile of 5 | 0.94 s | 1.26 s |
| of it, the editor writing the zone, median of 5 | 0.004 s | 0.018 s |
| of it, cargo finding nothing to build, median of 5 | 0.16 s | 0.14 s |
| Play on a level of 100 000 entities, to control, median of 5 | 1.95 s | 7.61 s |
| Play on a level of 100 000 entities, to control, 90th percentile of 5 | 2.22 s | 9.35 s |
| of it, the editor writing the level, median of 5 | 0.60 s | 2.68 s |
| of it, cargo finding nothing to build, median of 5 | 0.20 s | 0.24 s |

What it says: on the game's largest level, the zone, Play gives control well within the budget of
[ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md), in either build, so that the game stays a
process of its own. The level of 100 000 entities is within it in release, and far over it in
debug: the editor writing the level takes a third of the time, and the game's start, reading the
level and spawning it without optimizations, most of the rest. That level is the examples' yard with
100 000 entities more, a name for each of their 1 000 parents and a parent for each of the others,
none drawn: entities with a transform each would not fit one scene file, which holds at most
`slime_serialize::MAX_VALUES` values. In a headless Weston, the machine's Vulkan driver refuses the
window's surface, and the game draws with llvmpipe: Play on the zone took 8.6 s there, which
measures the CPU's drawing, not Play.

## M6c.5a, Ryzen 7 5800X

On 2026-10-29, from the commits of M6c.5a, on the CPU alone, in a release build, as
`cargo run --release -p slime-benches -- editor devui` measures them: the editor's hierarchy over
100 000 named entities, 1 000 roots of 99 children each, every root unfolded
([ADR-0167](adr/0167-the-hierarchy.md)), and the dev overlay's world panel, whose list now reads
again the names that changed at a rename rather than listing the world again, which M6b.8 measured
at 6.93 ms.

| Benchmark | Measure | Value |
|---|---|---|
| `editor.hierarchy.100k` | elements | 184 |
| `editor.hierarchy.100k` | a frame standing still, median of 15 | 4.4 µs |
| `editor.hierarchy.100k` | a frame standing still, 90th percentile of 15 | 4.6 µs |
| `editor.hierarchy.100k` | a frame scrolled by a row, median of 15 | 145.5 µs |
| `editor.hierarchy.100k` | a frame after an entity was renamed, median of 15 | 56.7 µs |
| `editor.hierarchy.100k` | a frame that runs a search, once the typing paused, median of 15 | 5.43 ms |
| `editor.hierarchy.100k` | a frame that runs a search, once the typing paused, 90th percentile of 15 | 6.08 ms |
| `editor.hierarchy.100k` | a frame after an entity was spawned, the rows made again, median of 15 | 1.02 ms |
| `devui.world_panel.100k` | a frame after an entity was renamed, median of 15 | 602.8 µs |
| `devui.world_panel.100k` | a frame after an entity was renamed, 90th percentile of 15 | 1.09 ms |

What it says: standing still, scrolled and after a rename, the hierarchy stays within the budget of
a list's frame of [ADR-0140](adr/0140-the-tree-of-elements.md), whatever the world's size, building
the rows in sight alone. A change of the world's structure makes the rows of the whole tree again,
about a millisecond at 100 000 entities, which a frame that spawns or reparents pays once. The
search reads every name, some 5 ms, once the typing paused: a frame that it runs in passes a list's
budget, and stays within one of a display at 60 Hz; it took 9 ms before it compared names without
copying them. The world panel's frame after a rename takes a tenth of what it did.

## M6c.5b, Ryzen 7 5800X

On 2026-10-29, on the CPU alone, in a release build, as
`cargo run --release -p slime-benches -- world.deep` measures them: a hierarchy 10 000 deep, each
entity the child of the one before (`benches/README.md`), before and after G32's walks of every
entity's ancestors were made linear ([ADR-0168](adr/0168-rearranging-the-hierarchy.md)). Before is
the world of `c2288c8` measured by the same benchmark.

| Measure | Before | After |
|---|---|---|
| 10 000 entities spawned, each the child of the one before, once | 122.95 ms | 5.62 ms |
| `World::save`, each moved, median of 5 | 337.16 ms | 60.33 ms |
| `World::interpolated_transform` of each, carried by its root, median of 20 | 160.81 ms | 432.2 µs |

What it says: each of the three walks cost the square of the hierarchy's depth, and costs it no
longer. What the save still takes is that of 10 000 entities changed, whatever their depth: a flat
world of as many, each moved, takes as long, and twice as many take some 2.3 times as long.

## M6c.5e, Ryzen 7 5800X, NVMe

On 2026-11-05, in a release build: a file of 1.6 KB, the size of the editor's layout file, written
whole as `slime_cook::write` writes it, a temporary file synced to the disk then renamed over the
file, 50 times in the user's home, on ext4 over an NVMe drive
([ADR-0174](adr/0174-layouts-kept-floating-panels-and-closed-panels.md)).

| Measure | Value |
|---|---|
| One write, median | 0.65 ms |
| One write, 90th percentile | 0.74 ms |
| One write, slowest | 0.96 ms |

What it says: a write costs a large part of a frame's budget at 240 Hz, so the editor writes its
layout once it has stood still for half a second, not at each change. Not measured on Windows.

## M6c.5e, CI's runners

GitHub's hosted runners, on 2026-11-04 and 2026-11-05. Each job's limit is 20 minutes
([ADR-0175](adr/0175-the-quality-bar.md)).

| Commit | Job | Time |
|---|---|---|
| `9edb2f8` | `check (headless)`, the longest Linux job | 16 min 52 s |
| `9edb2f8` | `windows` | 19 min 18 s |
| five runs of 2026-11-04 and 2026-11-05 | `windows` | from 13 min 44 s to 19 min 18 s, its tests' step varying most |

## M6c.Q1, CI's runners

GitHub's hosted runners, on 2026-11-05, commit `23d4027`, its run green.

| Job | Time |
|---|---|
| `windows` | 17 min 57 s |
| `check (headless)` | 11 min 44 s |
| `windows-release` | 10 min 53 s |
| `check (windowed)` | 5 min 15 s |
| `check (game)` | 4 min 35 s |

## M6c.N, CI's runners

GitHub's hosted runners, on 2026-11-05, commit `0a858f8`, its run green. The rename changed
comments in the cooker's sources, which the cooked zone's key holds: `check (game)` and `windows`
cooked the zone again, `windows`'s step taking 4 min 15 s in place of about 1 min, and `windows`
passed the limit of 20 minutes.

| Job | Time |
|---|---|
| `windows` | 21 min 43 s, its tests 14 min 11 s |
| `windows-release` | 16 min 44 s |
| `check (headless)` | 13 min 36 s |
| `check (game)` | 9 min 37 s |
| `check (windowed)` | 5 min 10 s |

## M6c.Q2a, CI's runners

GitHub's hosted runners, on 2026-11-05, commit `4164455`, its run green. The cooked zone came from
the cache: the cooker's packages did not change. The checks of rest
([ADR-0185](adr/0185-rest-checked-in-ci.md)) watch the editor in `check (windowed)`, in place of
the old check's 11 seconds, and Hunna Wood paused in `check (game)`, whose `drive` test took 54 s;
their own test runs in `check (headless)`.

| Job | Time |
|---|---|
| `windows` | 18 min 37 s, its tests 13 min 51 s |
| `check (headless)` | 14 min 18 s |
| `windows-release` | 14 min 11 s |
| `check (game)` | 7 min 10 s |
| `check (windowed)` | 5 min 23 s |

At rest, on the Ryzen 7 5800X in a headless Weston with lavapipe, before the milestone: the editor's
53 threads, 52 still and `blocking-1` switching 21 times in 10 s; Hunna Wood paused, its 16 threads
of lavapipe each switching about 500 times in 10 s. After it: no thread of either switches, runs,
starts or ends in 10 s.

## M6c.Q2b, Ryzen 7 5800X and RX 6900 XT

On 2026-11-05, from commit `f70f42a`, in a release build, the machine at rest, its desktop and
nothing else running. The editor's whole interface
([ADR-0186](adr/0186-counters-and-ratios-in-ci.md)) as
`cargo run --release -p slime-benches -- editor.window` and `-- editor.window.zone` measure it, at
3840 × 2160, on the RX 6900 XT through RADV, offscreen, without resizable BAR: each frame whole,
the core's part, the interface built, styled, laid out and gathered, then drawn through the
canvas; and its part building the interface. Of 15 frames, the two levels' frames in turn.

| Benchmark | Frame | Whole, median | Whole, 90th percentile | Built, median | Built, 90th percentile |
|---|---|---|---|---|---|
| `editor.window.zone` | standing still | 161.8 µs | 172.7 µs | 18.3 µs | 19.1 µs |
| `editor.window.zone` | after the selection changed | 183.3 µs | 193.4 µs | 38.8 µs | 47.7 µs |
| `editor.window.zone` | after a step | 471.6 µs | 667.7 µs | 323.9 µs | 393.8 µs |
| `editor.window.entities`, 10 000 | standing still | 198.6 µs | 277.1 µs | 26.9 µs | 33.6 µs |
| `editor.window.entities`, 10 000 | after the selection changed | 227.6 µs | 237.1 µs | 56.5 µs | 59.0 µs |
| `editor.window.entities`, 10 000 | after a step | 603.2 µs | 626.6 µs | 413.5 µs | 445.4 µs |
| `editor.window.entities`, 100 000 | standing still | 196.6 µs | 207.9 µs | 25.5 µs | 33.1 µs |
| `editor.window.entities`, 100 000 | after the selection changed | 229.7 µs | 259.1 µs | 56.0 µs | 57.1 µs |
| `editor.window.entities`, 100 000 | after a step | 598.7 µs | 664.9 µs | 411.5 µs | 469.4 µs |

The ratio of the interface built at 100 000 entities to at 10 000: 0.95, 0.99 and 1.00, standing
still, after the selection changed and after a step. `ui.draw.menu`'s `UiRenderer::draw` took
0.37 ms a frame at 1920 × 1080 and 3840 × 2160, as in [M6b.3b](#m6b3b-rx-6900-xt).

What it says: every frame stays within the budget of the editor's interface, 1.5 ms, the dearest,
after a step, under 0.7 ms at its 90th percentile; it does not grow from 10 000 entities to
100 000. A step builds the history, the status bar and the world panel again, two thirds of its
frame.

The same day, with other programs running, a load average of 5 to 11, the canvas's draw met
[ADR-0041](adr/0041-render-graph.md)'s stall: `ui.draw.menu` took 0.86 to 2.76 ms, and the
editor's frames 0.55 to 2.4 ms whole, a frame after a step passing the budget. Memory that the CPU
maps on this GPU is a window of 256 MiB, which other programs fill; uploads then stall
([ADR-0155](adr/0155-the-editors-design.md)). On lavapipe, in a release build, the zone's frames
took 0.56 ms whole standing still and 1.54 ms after a step.

In a debug build on lavapipe, pinned to 4 cores as CI's runners have, three runs of
`--counters editor.window.entities` gave ratios of 0.98 to 1.09, against the bound of 1.5. The
counters of every benchmark that counts, the zone's apart, took 61 to 65 s there, the same each
time, and the same as a release build's.

The CPU benchmarks on every core, and pinned to 4 cores (`taskset -c 0-3`, four cores without
their second threads), the proxy of the handheld tier ([ADR-0175](adr/0175-the-quality-bar.md)),
one run after the other, the machine at rest; medians:

| Benchmark | Measure | 16 logical CPUs | 4 cores |
|---|---|---|---|
| `ui.tree.editor` | a frame after one value of the inspector changed | 3.74 ms | 3.68 ms |
| `ui.tree.editor` | a frame with nothing changed | 529.3 µs | 496.1 µs |
| `ui.tree.list.100k` | a frame scrolled by 7 pixels | 68.7 µs | 67.9 µs |
| `devui.world_panel.100k` | a frame after an entity was renamed | 1.66 ms | 1.63 ms |
| `editor.hierarchy.100k` | a frame scrolled by a row | 278.7 µs | 279.7 µs |
| `editor.hierarchy.100k` | a frame that runs a search, once the typing paused | 5.54 ms | 5.76 ms |
| `editor.hierarchy.100k` | a frame after an entity was spawned, the rows made again | 1.16 ms | 1.22 ms |
| `world.transforms.100k` | `World::update_transforms` after 1 000 parents moved | 3.97 ms | 3.61 ms |
| `world.save.100k` | save of 100 000 entities, 1 000 moved | 7.01 ms | 7.25 ms |
| `world.undo.1k` | deleted then restored | 2.78 ms | 2.88 ms |
| `ui.text.labels.1k` | 1 000 labels laid out | 7.87 ms | 7.84 ms |

What it says: these paths run on one thread, and lose nothing on 4 cores. The proxy pins cores of
the Ryzen 7 5800X, faster than the handheld's, at its clock: it shows what needs more than 4
cores, not the handheld's times. The hierarchy's figures are those of M6c.5a, within some 15 %;
its search still passes a list's budget (M6c.Q3).

## M6c.Q2b, CI's runners

GitHub's hosted runners, on 2026-11-05, commit `7535915`, its run green. The cooked zone came from
the cache. `check (game)` now runs the benchmarks' counters
([ADR-0186](adr/0186-counters-and-ratios-in-ci.md)): 1 min 42 s, their build included, of which
1 min 37 s for every benchmark that counts, the zone's apart, and 2 s for the zone's.

| Job | Time |
|---|---|
| `check (headless)` | 17 min 12 s |
| `windows` | 16 min 47 s, its tests 12 min 36 s |
| `windows-release` | 10 min 59 s |
| `check (game)` | 8 min 45 s |
| `check (windowed)` | 5 min 12 s |

## M6c.Q2c, Ryzen 7 5800X

On 2026-11-05, in headless Westons 16 with lavapipe, the desktop running, a load average of 0.6
to 2.8. A hidden window rests ([ADR-0187](adr/0187-a-hidden-window-at-rest.md)).

Rest of a hidden game, as the `drive` test checks it: *Hunna Wood* minimized through
`minimize_game`, its game time running, in a debug build. With the old pacing, it was never still
for a second within 60 s: in its last second, each of its 16 threads of lavapipe switched 564 to
678 times, and its main thread 13 times. With the new one, none of its threads switches, runs,
starts or ends in 10 s. The `window` example minimized: its 38 threads switch not once in 10 s.

The frames' pacing while the window shows: the `window` example in a release build, 600 frames,
the first 10 left out, through `SLIME_TRACE` and `scripts/frame-pacing.py`, in a headless Weston
whose output refreshes at each rate (`--refresh-rate`). Intervals between the starts of frames:

| Display | Pacing | Frames a second | Mean | Standard deviation | 99th percentile | Longest |
|---|---|---|---|---|---|---|
| 50 Hz | old | 60.00 | 16.667 ms | 0.011 ms | 16.694 ms | 16.716 ms |
| 50 Hz | new | 50.00 | 20.000 ms | 0.036 ms | 20.110 ms | 20.203 ms |
| 60 Hz | old | 60.00 | 16.667 ms | 0.011 ms | 16.696 ms | 16.710 ms |
| 60 Hz | new | 60.00 | 16.667 ms | 0.009 ms | 16.693 ms | 16.773 ms |
| 144 Hz | old | 60.00 | 16.667 ms | 0.012 ms | 16.701 ms | 16.723 ms |
| 144 Hz | new | 60.00 | 16.667 ms | 0.071 ms | 16.694 ms | 17.879 ms |
| 240 Hz | old | 60.00 | 16.667 ms | 0.033 ms | 16.696 ms | 17.072 ms |
| 240 Hz | new | 60.00 | 16.667 ms | 0.014 ms | 16.697 ms | 16.778 ms |

What it says: where the display refreshes at least as often as the schedule asks, 60 times a
second by default, the frames keep [ADR-0021](adr/0021-frame-pacing.md)'s pacing, the compositor's
signal coming before the next frame is due. On a slower display, they follow it: 50 a second at
50 Hz, every image shown, where the old pacing drew 60, ten of which were never shown. A
compositor's signal that comes late shows as one longer interval, 17.9 ms once at 144 Hz.

On the owner's desktop the same day, its compositor and display, the RX 6900 XT through RADV, the
`window` example in a release build, 1 200 frames: 60.00 frames a second, intervals of a mean of
16.667 ms, a standard deviation of 0.010 ms, a 99th percentile of 16.694 ms and a longest of
16.713 ms, ADR-0021's pacing. *Hunna Wood* hidden there, in a release build: its threads switched
not once in 10 s, read from `/proc`: the compositor sends no signal to a hidden window.

## M6c.Q2c, CI's runners

GitHub's hosted runners, on 2026-11-05, commit `5002bf1`, its run green. The cooked zone came from
the cache. `check (game)`'s `drive` test watches the game minimized too
([ADR-0187](adr/0187-a-hidden-window-at-rest.md)): it took 68 s, against 54 s at M6c.Q2a.
`check (windowed)` starts one more headless Weston, at 240 Hz, for the windowed lifecycle test.

| Job | Time |
|---|---|
| `windows-release` | 13 min 57 s |
| `windows` | 11 min 49 s, its tests 9 min 8 s |
| `check (headless)` | 11 min 27 s |
| `check (game)` | 9 min 0 s |
| `check (windowed)` | 6 min 32 s |

## M6c.Q2d, Ryzen 7 5800X and RX 6900 XT

On 2026-11-06, the engine at commit `36f92bc`, in release builds, the machine at rest, its desktop
running ([ADR-0189](adr/0189-first-measures-of-time.md)); input to present with
`scripts/input-to-present.sh` as it stands after `07dd6c6`
([ADR-0191](adr/0191-input-in-sway-start-budgets-and-the-editors-rate.md)).

Warm starts, as `scripts/start-times.sh --gpu` measures them, in a headless Weston 16 through its
GL renderer, the RX 6900 XT through RADV, 20 runs each after 3 that warm the system's caches. The
log's times count from the engine's start, medians; `hyperfine`'s, from the process's launch,
means; a start is the log's time to the first image plus the launch:

| Program | Launch, `--help` | First image, log | Start | Level or zone drawn whole, log | Run of `--frames 1` whole |
|---|---|---|---|---|---|
| The editor, on the yard | 1.6 ms | 176.8 ms | 178.4 ms | 173.6 ms | 190.8 ms ± 3.8 ms |
| *Hunna Wood*'s editor, on its zone | 1.6 ms | 187.6 ms | 189.1 ms | 466.0 ms, from 419.0 to 499.6 ms | 218.2 ms ± 6.7 ms |
| *Hunna Wood*, to its zone drawn | 1.7 ms | 201.1 ms | 202.8 ms | 341.9 ms, from 259.5 to 418.5 ms | 368.2 ms ± 41.2 ms |

Where the editor's start goes, from one run's trace (`SLIME_TRACE`): the window made at 3 ms; the
GPU opened, 50 ms; the system's fonts read (`system_fonts`), 23 ms; the renderer's pipelines made,
45 ms; the first frame's interface built, styled and laid out, 31 ms, and drawn, 15 ms; presented
at 176 ms.

What it says: the editor's start passes its budget of 150 ms by some 30 ms, its steps run one after
the other; the zone opened in the editor passes its budget of 300 ms; *Hunna Wood* gives control
well within its second. The yard's few meshes load within the first frame. On lavapipe the same
day, the machine not at rest, the editor's first image came 440 to 490 ms after the engine's start,
over four series.

Input to present, as `scripts/input-to-present.sh --gpu` measures it, in a headless sway 1.12
whose output refreshes at 240 Hz, through its GL renderer: `wlrctl` gives a person's input, each
program run twice, once with 200 inputs apart, 100 ms from each other, pointer moves, clicks, the
wheel and keys in turn, once with 1 000 moves of the pointer in a row, about 180 a second. Two
runs of the script, the second while another program's fuzzing started on the machine; the
intervals from an input to the end of the present of the image after it:

| Program | Inputs | Images after an input | Median | 99th percentile | Longest | Budget |
|---|---|---|---|---|---|---|
| The editor, on the yard | apart | 148, 150 | 1.87 ms, 2.07 ms | 14.07 ms, 13.53 ms | 14.10 ms, 13.64 ms | 1 frame, 4.17 ms |
| The editor, on the yard | in a row | 554, 482 | 10.12 ms, 12.66 ms | 19.24 ms, 17.54 ms | 25.83 ms, 31.16 ms | 1 frame, 4.17 ms |
| *Hunna Wood* | apart | 99, 78 | 11.10 ms, 12.36 ms | 19.35 ms, 16.74 ms | the same | 2 frames, 33.3 ms |
| *Hunna Wood* | in a row | 220, 465 | 16.04 ms, 12.78 ms | 35.01 ms, 16.83 ms | 37.32 ms, 17.09 ms | 2 frames, 33.3 ms |

What it says: the editor answers an input apart within about 2 ms at the median, but its 99th
percentile, apart and in motion, passes one frame of 4.17 ms by three to four: an input waits for
the next frame of a schedule of 60 a second, 16.7 ms ([`debt.md`](debt.md)). *Hunna Wood*'s inputs
wait for its frames at 60 a second, within its two frames but in one run in motion, where a frame
of 131 ms took its 99th percentile to 35.0 ms; it is to be measured again on a machine at rest.
Fewer than 100 images give their longest as the 99th percentile.

Variable refresh rates, on the owner's desktop, the `window` example in a release build, 1 200
frames, read by `scripts/frame-pacing.py`:

| Display | Adaptive sync | Frames a second | Mean | Standard deviation | 99th percentile | Longest |
|---|---|---|---|---|---|---|
| DP-2, 3840 × 2160 at 240 Hz | not reported on | 60.00 | 16.667 ms | 0.029 ms | 16.710 ms | 16.959 ms |
| DP-3, 1920 × 1080 at 180 Hz | on | 60.00 | 16.667 ms | 0.024 ms | 16.700 ms | 16.954 ms |

What it says: with adaptive sync on, the frames keep [ADR-0021](adr/0021-frame-pacing.md)'s pacing
as [ADR-0187](adr/0187-a-hidden-window-at-rest.md) leaves it. At 60 frames a second, which divides
both refreshes, a display that did not follow the frames would pace them the same: whether it
follows them is judged at another rate (M6c.Q2f). What the display shows the engine does not see.

## M6c.Q2d, cold starts, RX 6900 XT

On 2026-11-06, the engine at commit `36f92bc`, `scripts/start-times.sh --gpu --cold --runs 5` run by
the owner, the machine at rest: the system's caches dropped before each run
([ADR-0192](adr/0192-the-budget-of-a-cold-start.md)). Medians of the logs, means of `hyperfine`:

| Program | Launch, `--help` | First image, log | Start | Level or zone drawn whole, log | Run of `--frames 1` whole |
|---|---|---|---|---|---|
| The editor, on the yard | 16.3 ms | 395.5 ms | 411.8 ms | 396.3 ms | 447.7 ms ± 5.5 ms |
| *Hunna Wood*'s editor, on its zone | 15.5 ms | 419.7 ms | 435.2 ms | 733.3 ms | 486.7 ms ± 6.4 ms |
| *Hunna Wood*, to its zone drawn | 16.3 ms | 387.0 ms | 403.3 ms | 667.6 ms | 724.9 ms ± 17.0 ms |

What it says: a cold start costs some 230 ms more than a warm one, the program, its libraries, the
drivers and the data read from the disk again; the editor's is within its 500 ms.

## M6c.Q2d, CI's runners

GitHub's hosted runners, on 2026-11-06, commit `649467c`, its run green. The cooked zone came from
the cache. `check (windowed)` runs the editor once to its first image and once to its level drawn
whole ([ADR-0189](adr/0189-first-measures-of-time.md)).

| Job | Time |
|---|---|
| `windows` | 17 min 48 s, its tests 13 min 21 s |
| `windows-release` | 14 min 38 s |
| `check (headless)` | 12 min 14 s |
| `check (game)` | 7 min 12 s |
| `check (windowed)` | 5 min 38 s |

## M6c.Q2e, Ryzen 7 5800X and RX 6900 XT

On 2026-11-06, in release builds, [ADR-0193](adr/0193-first-measures-of-resources.md)'s measure:
`scripts/resources.sh --gpu`, a headless Weston 16 at 3840 × 2160, scale 3, through its GL renderer,
the RX 6900 XT through RADV; `hunnawood-editor` on a copy of the zone, its window 3840 × 2400,
driven through the MCP server. Memory at commit `06ddcce`, the machine busy with a virtual machine;
power from a run an hour before, the desktop at rest, its measure the same but for its reading of
the driver's sizes, which gave the same figures.

Memory and video memory, read at rest, from one run; video memory asked of the driver, of it
resident, and the system's memory that the GPU maps:

| When | Resident | Anonymous | Files | Video memory asked | Of it resident | Mapped from the system's |
|---|---|---|---|---|---|---|
| The editor on the zone | 172.9 MiB | 106.7 MiB | 62.6 MiB | 986.0 MiB | 858.0 MiB | 209.7 MiB |
| After a step and 300 undos and redos | 187.2 MiB | 108.2 MiB | 62.6 MiB | 986.1 MiB | 858.1 MiB | 223.2 MiB |
| After 100 undos and redos more | 187.2 MiB | 108.2 MiB | 62.6 MiB | 986.1 MiB | 858.1 MiB | 223.2 MiB |
| After a Play, which built the game | 187.0 MiB | 108.3 MiB | 62.3 MiB | 986.1 MiB | 858.1 MiB | 223.2 MiB |
| After 10 Plays more | 187.8 MiB | 109.2 MiB | 62.3 MiB | 986.1 MiB | 858.1 MiB | 223.2 MiB |
| *Hunna Wood* on its zone, running at 60 frames a second | 158.0 MiB | 98.3 MiB | 56.1 MiB | 1 236.5 MiB | 1 108.5 MiB | 218.8 MiB |

Over five runs, the editor on the zone at rest was 123.6 to 172.9 MiB resident, its anonymous
memory alone changing, and *Hunna Wood* 144.7 to 158.0 MiB. The editor's video memory asked was
986 MiB, but 858 MiB in one run after its undos, 128 MiB fewer, as much less mapped: most likely a
block of the GPU's memory given back once the uploads ended, which was not traced.

The editor's heap, from `scripts/resources.sh --gpu --heaptrack`, at the end of each phase:

| Run | At rest | After the step and 300 undos and redos | After the undos and redos measured | After a Play | After the Plays measured |
|---|---|---|---|---|---|
| 100 undos and redos, 10 Plays | 21.3 MiB | 28.7 MiB | 28.8 MiB | 28.8 MiB | 29.1 MiB |
| `--undos 1000 --plays 40` | 21.3 MiB | 26.1 MiB | 28.8 MiB | 28.7 MiB | 29.3 MiB |
| `--plays 100` | 25.4 MiB | 32.8 MiB | 32.8 MiB | 32.8 MiB | 33.3 MiB |

Its peak, as the zone loads, 46.2 MiB. Over the 100 Plays, the heap rose by 0.4 to 0.7 MiB within
the first ten, then moved between 33.2 and 33.6 MiB to the last.

The GPU's power, amdgpu's sensor, 120 readings over 30 s each:

| What runs | Mean | Lowest | Highest |
|---|---|---|---|
| Nothing but the desktop and Weston, before | 34.0 W | 33.0 W | 41.0 W |
| The editor at rest on the zone | 34.6 W | 33.0 W | 35.0 W |
| *Hunna Wood* at 60 frames a second, 1 804 frames in the 30 s | 71.6 W | 69.0 W | 75.0 W |
| Nothing but the desktop and Weston, after | 34.1 W | 33.0 W | 35.0 W |

What it says:

- The editor's memory, at most 173 MiB, within its budget of 256 MiB; a third of it, the program
  and its libraries mapped from their files. Its heap at rest is some 23 MiB, at its peak 46 MiB:
  much of the rest of its anonymous memory is the allocator's, which keeps in part, more or less
  from one run to the next, what the threads that load the zone freed.
- Its video memory, 986 MiB asked, within its 1.5 GiB.
- Neither undos and redos nor Play grow the heap. The first few hundred undos and redos fill some
  7 MiB once: wgpu's pool of Vulkan command buffers, which `radv_create_cmd_buffer` allocates for
  each pass that the viewport submits (`slime-gfx`'s `pass.rs`), grows as the frames that they
  draw follow each other fast, and is never shrunk. The first ten Plays fill under 1 MiB once.
- The editor at rest adds no power that the sensor sees: 34.6 W against 34.0 W before and 34.1 W
  after, within its watt; at another run the same day, 34.4 W against 34.5 W.
- *Hunna Wood* at 60 frames a second at 3840 × 2160 adds 37.6 W to the desktop's, within its
  budget of 50 W; the power counts Weston's composition of each image. Runs on the busy machine
  gave 37 to 39 W.
- The game's memory, at most 158 MiB, and its video memory, 1.2 GiB, are far within the high
  tier's 24 GB and 12 GB, and the handheld tier's 8 GB and 4 GB; a tier's own machine is not
  measured.

## M6c.Q2e, CI's runners

GitHub's hosted runners, on 2026-11-06, commit `5d9ee34`, its run green. The cooked zone came from
the cache. `heaptrack` entered the development shell: each Linux job fetched it in about 5 s more,
32 s against 27 s for `check (headless)`. That job's other steps, which the milestone did not
change, ran 24 to 49 % slower than on `649467c`: a slower runner.

| Job | Time |
|---|---|
| `windows` | 18 min 17 s |
| `check (headless)` | 16 min 40 s |
| `windows-release` | 14 min 9 s |
| `check (game)` | 9 min 2 s |
| `check (windowed)` | 4 min 45 s |

## M6d.0a, the renderer against the canvas, and a program on uwui

What [ADR-0180](adr/0180-one-canvas-and-the-frameworks-foundations.md) asks to be measured first,
on 2026-11-06, the tree of `aac4607` with the milestone's scripts: an AMD Ryzen 7 5800X, 16
threads, 32 GB, an RX 6900 XT on RADV, Linux 7.3, Rust 1.99.0. Memory in MiB, as elsewhere in this
file; a binary's size also in MB of 10⁶ bytes, the unit of uwui's budget.

### What the renderer costs an application that draws only interfaces

[`scripts/canvas-cost.sh`](../scripts/canvas-cost.sh) `--gpu`, 20 runs a part, each in a process
of its own, after the device and a target of 1280 × 800 are made. The median, and the range.

| | The device alone | The canvas alone | The renderer, its canvas in it |
|---|---|---|---|
| Built, and a first canvas drawn | 0.2 ms (0.1–0.2) | 7.5 ms (7.2–9.1) | 37.8 ms (36.6–42.7) |
| Resident memory added | 0.4 MiB (0.4–0.4) | 1.8 MiB (1.8–1.8) | 3.4 MiB (3.4–3.5) |
| GPU memory that wgpu allocates | 0 (0–0) | 9 KiB (9–9) | 117 KiB (117–117) |
| Video memory that the driver gives | 0 (0–0) | 2.6 MiB (2.6–2.6) | 2.7 MiB (2.7–2.7) |

The device itself, with the target, before either: opened in 42.7 ms (41.1–47.7), 51.5 MiB
resident, and 195.1 MiB of video memory, every run.

The times are those of a warm start. Each part's runs follow one that fills the system's caches,
the driver's cache of compiled shaders on disk among them: `slime-gfx` keeps no cache of pipelines
of its own, and a first start on a machine compiles every shader.

On lavapipe, which compiles the shaders on the CPU, without video memory: the device opened in
29.9 ms, with the target 59.0 MiB resident; the canvas built in 3.4 ms, with a first canvas drawn
in 12.1 ms, and 26.8 MiB; the renderer built in 133.7 ms, with a first canvas drawn in 142.1 ms,
and 66.4 MiB.

What it says:

- The renderer costs an application some 30 ms more than the canvas alone at its start, on the
  high tier's GPU, and 1.6 MiB more resident: its pipelines, layouts and the BRDF table, computed
  on the GPU. Its video memory is almost the canvas's.
- The device costs more than either: some 43 ms, 52 MiB resident, and 195 MiB of video memory.
  wgpu 30's default hint, `MemoryHints::Performance`, which `slime-gfx`'s `Gpu::new` keeps, makes
  the first allocations reserve a block of 128 MiB of the GPU's memory and one of 64 MiB of memory
  that the CPU sees (`wgpu-hal`'s `AllocationSizes`); a small application never fills them. The
  debt is in [`debt.md`](debt.md).
- On lavapipe, the canvas's and the renderer's memory is the CPU's: their shaders compiled by
  LLVM.

### What a program on uwui compiles

[`scripts/uwui-program.sh`](../scripts/uwui-program.sh): the `widgets` example, which draws the
editor's widgets with uwui through the renderer's canvas, built in a package of its own with the
workspace's lock and profiles, from an empty target directory, on 16 threads. With `slime-app`'s
features as the example has them, `development` and `accessibility`, and with none, as a program
shipped without the development channel.

| | `development`, `accessibility` | No feature |
|---|---|---|
| Crates compiled, the engine's among them | 271, 15 | 207, 14 |
| `slime-render`, `slime-world`, `slime-editor` | compiled, compiled, not | compiled, compiled, not |
| Debug build, from nothing / after a change of its code | 32.2 s / 1.2 s | 28.1 s / 1.0 s |
| Release build, from nothing / after a change of its code | 55.3 s / 2.5 s | 43.0 s / 2.2 s |
| Release binary / stripped | 32.0 MB / 24.7 MB | 25.3 MB / 19.7 MB |

What it says:

- A program on uwui compiles the renderer and the world today, which ADR-0180 wants it to leave
  out: `uwui-render` draws through a `Renderer`, and `slime-app` depends on `slime-world`.
- Its binary, stripped and without the development channel, is within uwui's budget of 20 MB by
  0.3 MB; with the channel, over it.

### CI's runners

GitHub's hosted runners, on 2026-11-06, commit `0a1ba51`, its run green.

| Job | Time |
|---|---|
| `windows` | 15 min 4 s |
| `windows-release` | 14 min 4 s |
| `check (headless)` | 10 min 55 s |
| `check (game)` | 9 min 1 s |
| `check (windowed)` | 6 min 51 s |
| `links` | 18 s |

## M6d.0b, a program on uwui's start and rest, measured from outside

What [ADR-0180](adr/0180-one-canvas-and-the-frameworks-foundations.md) asks that M6d.0a left: an
application's start to its first useful frame, and its rest under Wayland and under Xwayland. The
`widgets` example in a release build, the tree of `5390db2`, measured from outside by uwui's
[`scripts/measure.py`](https://github.com/HoneySlatt/uwui/blob/main/scripts/measure.py)
([ADR-0200](adr/0200-applications-measured-from-outside.md)), on 2026-11-06: an AMD Ryzen 7 5800X,
an RX 6900 XT on RADV, Linux 7.3; sway 1.12 without a screen, one output of 3840 × 2160 at 240 Hz,
scale 1.5, the window alone on it, drawn on the GPU. 10 starts after one not counted, the median
and the range. Memory in MB of 10⁶ bytes, as uwui's figures. The command, inside the development
shell, from the repository's root, with uwui's repository at `<uwui>`:

```sh
env -u VK_LOADER_DRIVERS_SELECT -u VK_LOADER_DRIVERS_DISABLE -u VK_DRIVER_FILES \
    python3 -I <uwui>/scripts/measure.py --xwayland --power \
    --steps <uwui>/scripts/hello-world.steps target/widgets -- target/release/examples/widgets
```

Its steps wait for a still frame that never comes, 60 s, before the memory and the rest.

| | `widgets` |
|---|---|
| Launch to the first frame shown | 109.1 ms (108.0–111.3) |
| Memory some 63 s after the launch: resident / proportional / video | 75.3 / 44.7 / 345.4 MB |
| 60 s without input, Wayland: frames, the processor, the compositor | 3,600, 3.0 % of a core, 1.9 % |
| The same, Xwayland | 3,600, 3.5 % of a core, 1.9 % |
| The same: wake-ups, Wayland / Xwayland | 7,414 / 37,023 |

What it says:

- The first frame comes within the editor's budget of 150 ms, and 9 ms over uwui's 100 ms for a
  typical application ([uwui's quality bar](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#budgets)).
- The example is never at rest: its two progress bars, "cooking textures" and "indexing", move
  without end, and it draws 60 frames a second, its schedule's rate. No still frame follows to
  compare its frames with: whether its first frame is its first useful one is not checked. Its
  rest is to be measured on an application that rests; the `window` example's is checked in CI
  ([ADR-0187](adr/0187-a-hidden-window-at-rest.md)).
- Under Xwayland it wakes five times as often for the same frames.

## M6d.1b2, the bench on the control

The bench's figures, the control's, are uwui's: its
[`docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#the-control-on-m6d1bs-bench-2026-11-06).
Here, CI's times for the change kept on `main`, the benchmarks' allocator counting bytes.

### CI's runners

GitHub's hosted runners, on 2026-11-06, commit `db1821e`, its run green.

| Job | Time |
|---|---|
| `windows` | 19 min 21 s |
| `windows-release` | 14 min 20 s |
| `check (headless)` | 13 min 45 s |
| `check (windowed)` | 6 min 44 s |
| `check (game)` | 5 min 57 s |

`windows` came within 39 s of the limit of 20 minutes, 4 min 17 s longer than at M6d.0a's
`0a1ba51`; why is not established. Its lack of room is known debt
([`debt.md`](debt.md)).

## M6d.1b3a, three models on the bench

The bench's figures, the four models', are uwui's: its
[`docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#three-models-beside-the-control-on-m6d1bs-bench-2026-11-07).
The milestone kept only documents on `main`, which run CI's job of documents alone: no code
changed there, so the jobs of code did not run.

GitHub's hosted runners, each run green:

| Commit | Job | Time |
|---|---|---|
| `2db2af2`, ADR-0203, on 2026-11-06 | `docs` | 24 s |
| `30312b7`, the milestone marked done, on 2026-11-07 | `docs` | 23 s |
| uwui's `732286f`, its pilots replaced, on 2026-11-06 | uwui's `ci` | 28 s |
| uwui's `66e6d8c`, the figures, on 2026-11-07 | uwui's `ci` | 25 s |

## M6d.1c1, signals named across a reload

The prototype's figures are uwui's: its
[`docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#signals-named-across-a-reload-2026-11-07).
The prototype lives on the branch `m6d1c-bench`, which runs no CI; `main` kept only documents,
which run CI's job of documents alone.

GitHub's hosted runners, on 2026-11-07, each run green:

| Commit | Job | Time |
|---|---|---|
| `8ee62fe`, ADR-0210 and the milestone marked done | `docs` | 18 s |
| uwui's `47ad601`, the figures | uwui's `ci` | 21 s |

## M6d.1c2, the interface's executor and other threads

The prototype's figures are uwui's: its
[`docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#the-interfaces-executor-and-other-threads-2026-11-07).
The prototype lives on the branch `m6d1c-bench`, which runs no CI. `main` took ADR-0211 and the
milestone marked done, pushed with the new logo, whose README ran CI whole.

GitHub's hosted runners, on 2026-11-07, each run green:

| Commit | Job | Time |
|---|---|---|
| `fde479c`, the logo, over `a78ffce`, ADR-0211 | `check (headless)` | 13 min 32 s |
| | `check (windowed)` | 5 min 11 s |
| | `check (game)` | 8 min 1 s |
| | `windows` | 15 min 57 s |
| | `windows-release` | 13 min 19 s |
| | Docs' `links` | 17 s |
| uwui's `689b62a`, the figures | uwui's `check` | 19 s |

## M6d.1c3a, a function's body reloaded through subsecond

The prototype's figures are uwui's: its
[`docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#a-functions-body-reloaded-through-subsecond-2026-11-07).
The prototype lives on the branch `m6d1c-bench`, which runs no CI. `main` took ADR-0212 and the
milestone marked done, documents alone, which run the `docs` job.

GitHub's hosted runners, on 2026-11-07, each run green:

| Commit | Job | Time |
|---|---|---|
| `89aa5b5`, ADR-0212 and the milestone marked done | Docs' `links` | 22 s |
| uwui's `7e84e62`, the figures | uwui's `check` | 20 s |

## Compiling, Ryzen 7 5800X

Commit `c6ce9dd`, on 2026-11-07: the times of compiling, against the budgets of
[ADR-0215](adr/0215-the-engine-measured-against-the-market.md), whose figures
[`ROADMAP.md`](ROADMAP.md#budgets) holds. An AMD Ryzen 7 5800X, 16 logical CPUs, inside
`nix develop -c`, from an empty target directory, each row after the one above it, debug builds
unless the row says otherwise. Each change of code was made on a copy of the repository in `/tmp`:
rustc reuses its work when only a file's date changes, so a run with `touch` alone, 3.5 s, does not
stand for a real change. The benchmarks are not measured.

| Build | Time | Crates compiled |
|---|---|---|
| `cargo build --workspace`, from nothing | 53.9 s (56.8 s on a second run) | 268 |
| then `cargo build -p hunnawood` | 31.3 s | 79 |
| then `cargo build --example editor` | 36.9 s | 130 |
| a function's body changed in the game, `-p hunnawood` | 1.93 s | 1 |
| a function's body changed in `slime-world`, `--example editor` | 3.05 s | 7 |
| the same change, `-p hunnawood` | 2.61 s | 5 |
| a public function added to `slime-world`, `--example editor` | 2.90 s | 7 |
| a public function added to `slime-input`, `--example editor` | 3.16 s | 8 |
| `cargo build --release -p hunnawood` | 70 s | 204 |

What it says:

- A change of a game's code to the game running again, in a debug build: over its budget. The
  game builds again, then Play gives control in the time that [M6c.4d](#m6c4d-rx-6900-xt)
  measured, about 3.1 s in all ([`debt.md`](debt.md)).
- The game and the editor compile crates again after the workspace's build, as their features
  differ from the workspace's: each engine crate is compiled once per set of features, and a change
  in `slime-world` is compiled for the editor and for the game apart.
- The editor built again after a change of an engine crate: within its budget.
- The whole engine from nothing, in a debug build: within its budget.
- A game's build for shipping: not measured, as there is no profile for shipping; the game's release
  build above is not one.
- A game's code reloaded hot, its state kept: not measured, as it does not exist yet (Stage 8).
- Unreal, Unity and Godot's times are not measured yet: the lead over them waits for Stage 6e
  ([ADR-0220](adr/0220-the-roadmap-reshaped-by-the-market.md)).

## M6d.1c3b, the rivals' reloads timed

The rivals' figures are uwui's: its
[`docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#the-rivals-reloads-2026-11-07).
`main` took the milestone marked done, a document alone, which runs the `docs` job.

GitHub's hosted runners, on 2026-11-07, each run green:

| Commit | Job | Time |
|---|---|---|
| `8c75a4b`, the milestone marked done | Docs' `links` | 18 s |
| uwui's `ab4c7df`, its roadmap in line with ADR-0213 to ADR-0220 | uwui's `check` | 20 s |
| uwui's `290bdd9`, the step `save` and the figures, pushed with `cdfab94` | uwui's `check` | 21 s |

## C1, before, Ryzen 7 5800X

Measured with [`scripts/compile-times.sh`](../scripts/compile-times.sh) on a clone of the commit
before C1, its Play building the examples' `world` as it then did, with `-p slime-examples`. The
processors were idle and no other build ran; the load average before the run was what remained of
an earlier build. "Packages compiled" counts the packages that cargo printed as compiled, the
library and the program of one package counted once.

Commit `4b164dd`, on 2026-11-08: AMD Ryzen 7 5800X 8-Core Processor,
16 logical CPUs, rustc 1.99.0, no flags of rustc set;
runs: 7 of each change, 3 of the workspace and 1 for shipping from nothing;
load average 0.90 1.64 3.45 before, 10.86 6.80 5.29 after.

| Step | Median | Range | Packages compiled |
|---|---|---|---|
| the workspace from nothing, `--workspace` | 35.43 s | 34.63 to 36.23 s | 268 |
| then the game's editor, `-p hunnawood-editor` | 23.69 s | one run | 90 |
| then the game as Play builds it, `-p hunnawood --bin hunnawood` | 2.00 s | one run | 1 |
| then the examples' editor, `--example editor` | 24.08 s | one run | 129 |
| then its game as Play builds it, `-p slime-examples --example world` | 20.72 s | one run | 42 |
| a function's body changed in the game, `-p hunnawood --bin hunnawood` | 1.42 s | 1.25 to 1.52 s | 1 |
| a function's body changed in `slime-world`, `-p hunnawood-editor` | 2.34 s | 2.03 to 4.09 s | 8 |
| then the game as Play builds it | 1.01 s | 0.99 to 1.07 s | 1 |
| a function's body changed in `slime-world`, `--example editor` | 2.31 s | 2.15 to 2.38 s | 7 |
| then its game as Play builds it | 2.37 s | 2.24 to 3.15 s | 7 |
| a public function added to `slime-input`, `-p hunnawood-editor` | 2.43 s | 2.22 to 2.49 s | 9 |
| the game in release from nothing, no profile for shipping, `--release -p hunnawood --no-default-features` | 38.73 s | one run | 198 |

Play's time to control on *Hunna Wood*'s zone, as `cargo run -p slime-benches -- play.zone`
measures it ([M6c.4d](#m6c4d-rx-6900-xt)), in a window on the owner's desktop drawn by the RX 6900
XT, in a debug build: 1.11 s, median of 5, 1.24 s at the 90th percentile; of it, cargo finding
nothing to build 0.14 s.

What it says:

- *Hunna Wood*'s editor and the game that its Play builds share every engine crate: only the game's
  program is built again.
- The examples' editor and the `world` example that its Play builds share none: each engine change
  is compiled twice.
- A function's body changed in the game: 1.42 s to build, then 0.97 s for Play to give control
  without cargo's own look, about 2.4 s, over the budget of 2 s.
- The same steps, measured once each on 2026-11-07 ([Compiling](#compiling-ryzen-7-5800x)), took
  53.9 s, 1.93 s and 70 s where these took 35.4 s, 1.42 s and 38.7 s, on the same code. The cause
  of the gap is not known: those runs were single, and their machine's load was not recorded.
  ADR-0220's 3.1 s rested on them.

## C1, after, Ryzen 7 5800X

Measured with [`scripts/compile-times.sh`](../scripts/compile-times.sh), after
[ADR-0221](adr/0221-compiling-within-its-budgets.md)'s changes, in the development shell, which links
with mold; [before](#c1-before-ryzen-7-5800x). The load average before the run was what remained of
the run before it.

Commit `4b164dd`, with the working tree's changes, on 2026-11-08: AMD Ryzen 7 5800X 8-Core Processor,
16 logical CPUs, rustc 1.99.0, Linux's `-C link-arg=-fuse-ld=mold`;
runs: 7 of each change, 3 of the workspace and 3 for shipping from nothing;
load average 4.19 5.61 4.98 before, 5.03 7.25 6.90 after.

| Step | Median | Range | Packages compiled |
|---|---|---|---|
| the workspace from nothing, `--workspace` | 61.96 s | 60.95 to 62.70 s | 268 |
| then the game's editor, `-p hunnawood-editor` | 36.44 s | one run | 90 |
| then the game as Play builds it, `-p hunnawood --bin hunnawood` | 1.83 s | one run | 1 |
| then the examples' editor, `--example editor` | 49.68 s | one run | 129 |
| then its game as Play builds it, `--workspace --example world` | 2.30 s | one run | 1 |
| a function's body changed in the game, `-p hunnawood --bin hunnawood` | 1.05 s | 0.98 to 1.25 s | 1 |
| a function's body changed in `slime-world`, `-p hunnawood-editor` | 1.71 s | 1.63 to 1.94 s | 8 |
| then the game as Play builds it | 0.73 s | 0.71 to 0.96 s | 1 |
| a function's body changed in `slime-world`, `--example editor` | 1.82 s | 1.77 to 3.12 s | 7 |
| then its game as Play builds it | 0.86 s | 0.84 to 0.98 s | 1 |
| a public function added to `slime-input`, `-p hunnawood-editor` | 1.96 s | 1.87 to 2.36 s | 9 |
| the game for shipping from nothing, `--profile shipping -p hunnawood --no-default-features` | 86.70 s | 85.67 to 87.00 s | 198 |

Play's time to control on the zone, measured as before, after these changes: 0.89 s, median of 5,
0.90 s at the 90th percentile; of it, cargo finding nothing to build 0.12 s.

Against the budgets of [ADR-0215](adr/0215-the-engine-measured-against-the-market.md):

| Budget | Before | After |
|---|---|---|
| A change of a game's code to the game running again, 2 s: the build, then Play's time to control without cargo's own look | 1.42 s and 0.97 s, about 2.4 s, over | 1.05 s and 0.77 s, about 1.8 s, within; 2.0 s with the slowest build of 7 |
| The editor built again after a change of an engine crate, 10 s | 2.34 s | 1.71 s |
| The whole engine from nothing, 3 min | 35.4 s | 62.0 s |
| A game's build for shipping, 10 min | no profile for shipping | 86.7 s |
| Reloaded hot, its state kept, under 1 s | does not exist (Stage 8) | does not exist (Stage 8) |
| Four times less waiting than Unreal, Unity and Godot | not measured (Stage 6e) | not measured (Stage 6e) |

What it says:

- The game's change runs again within its budget, as an estimate: the sum of two medians, not one
  interval timed end to end. With rustc's own lld, outside the shell, the trials below give about
  2.2 s, over.
- The examples' editor and the `world` that its Play builds now share every engine crate: Play
  builds one package after an engine change, in 0.86 s rather than 2.37 s.
- The workspace from nothing takes about 1.75 times as long, the dependencies optimised.
- The game built for shipping ran: 120 frames of its zone, its run saved to a file of its own.
- `scripts/uwui-program.sh`, run after these changes, its dependencies optimised and the engine's
  crates not: 271 crates, 15 of the engine's; 65.3 s from nothing in debug, 0.8 s after a change of
  its own code; 56.1 s from nothing in release; its release binary 31.9 MB, 24.1 MB stripped.

### The trials

The workspace from nothing, one run each, and Play's time to control on the zone, median of 5,
dependencies optimised at each level in debug builds, rustc's own lld:

| `opt-level` of the dependencies | The workspace from nothing | Play's time to control | 90th percentile |
|---|---|---|---|
| none | 33.1 s | 1.11 s | 1.24 s |
| 1 | 63.7 s | 0.90 s | 1.06 s |
| 2 | 75.2 s | 0.94 s | 0.96 s |
| 3 | 80.0 s | 0.95 s | 1.03 s |

The linkers, each on the whole of `scripts/compile-times.sh`, 7 runs of each change and 1 of the
workspace from nothing, dependencies at level 1, one run after the other; medians, with their range.
That version of the script let the examples' steps leak into the steps after them: only the steps
that they did not reach are given.

| Step | rustc's lld | mold 2.42.1 | wild 0.10.0 |
|---|---|---|---|
| a function's body changed in the game | 1.44 s, 1.21 to 1.97 s | 0.97 s, 0.94 to 1.46 s | 0.99 s, 0.94 to 1.09 s |
| a function's body changed in `slime-world`, the game's editor | 1.86 s | 1.63 s | 1.74 s |
| then the game as Play builds it | 0.90 s | 0.69 s | 0.72 s |
| the workspace from nothing | 58.8 s | 61.0 s | 62.3 s |
| the game in release from nothing | 38.3 s | 38.7 s | 38.6 s |

### CI's runners

GitHub's hosted runners, on 2026-11-08. On `fb2be2a`, `check (windowed)` failed: its `timeout`
of 120 s bounded the examples' build with their run, and the build, its dependencies optimised,
outlasted it; `8c3b9dc` builds them first. The other jobs of `fb2be2a` were green; every job of
`8c3b9dc` is.

| Job | `fb2be2a`, the changes | `8c3b9dc`, the checks' builds first |
|---|---|---|
| `check (headless)` | 15 min 55 s | 23 min 2 s |
| `check (windowed)` | failed after 3 min 29 s | 9 min 9 s |
| `check (game)` | 9 min 30 s | 9 min 50 s |
| `windows` | 19 min 52 s | 18 min 52 s |
| `windows-release` | 11 min 43 s | 16 min 0 s |
| Docs' `links` | 18 s | 17 s |

`check (headless)` ran the same code on both: on `8c3b9dc`, each of its steps took 1.4 to 1.6 times
as long, the runner slower, which took it over its limit of 20 minutes.

## M6d.1d1, themes and styles written in code

A style sheet's change seen is uwui's figure: its
[`docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#a-style-sheets-change-seen-2026-11-09).
The prototype lives on the branch `m6d1c-bench`, which runs no CI. `main` took ADR-0222 to
ADR-0224, documents alone, and `rmcp` raised to 3.9.0, past RUSTSEC-2026-0457 and
RUSTSEC-2026-0462, which ran every job.

GitHub's hosted runners, on 2026-11-09, each run green, on `8ff4e59`:

| Job | Time |
|---|---|
| `check (headless)` | 23 min 23 s |
| `check (windowed)` | 11 min 19 s |
| `check (game)` | 9 min 20 s |
| `windows` | 23 min 0 s |
| `windows-release` | 16 min 1 s |
| Docs' `links` | 20 s |
| uwui's `check`, on its `e7cec78` | 18 s |

`check (headless)` and `windows` went past their limit of 20 minutes, the cooked zone taken from its
cache: the runners' speed alone, which [`debt.md`](debt.md) follows under "CI's jobs and their
limit of 20 minutes".
