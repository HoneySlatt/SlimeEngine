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

## M6d.1d2a, layouts, their notation and their reload

A layout's change seen, and the notations counted, are uwui's figures: its
[`docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#a-layouts-change-seen-2026-11-09).
The prototype lives on the branch `m6d1c-bench`, which runs no CI. `main` took ADR-0225 and the
documents it changes alone, which run the Docs workflow and not CI's jobs; the engine's code is as
M6d.1d1 left it.

GitHub's hosted runners, on 2026-11-09, each run green:

| Job | Time |
|---|---|
| Docs' `links`, on `20444e7` | 19 s |
| uwui's `check`, on its `7e409e6` | 20 s |

## M6d.1d2b1, an application's start and its look

A milestone of decisions alone
([ADR-0226](adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)): nothing was
measured. `main` took ADR-0226 and the documents it changes alone, which run the Docs workflow and
not CI's jobs; the engine's code is as M6d.1d1 left it.

GitHub's hosted runners, on 2026-11-09, each run green:

| Job | Time |
|---|---|
| Docs' `links`, on `99191d2` | 17 s |
| uwui's `check`, on its `eb5e41e` | 16 s |

## M6d.1d2b2, the time that motion reads

A milestone of decisions alone ([ADR-0227](adr/0227-the-time-that-motion-reads.md)): nothing was
measured. `main` took ADR-0227 and the documents it changes, with a comment of `slime-app`'s
rustdoc, which ran every job; the engine's code is as M6d.1d1 left it.

GitHub's hosted runners, on 2026-11-10, each run green, on `3a8e42e`:

| Job | Time |
|---|---|
| `check (headless)` | 22 min 56 s |
| `check (windowed)` | 11 min 16 s |
| `check (game)` | 11 min 55 s |
| `windows` | 23 min 25 s |
| `windows-release` | 15 min 52 s |
| Docs' `links` | 21 s |
| uwui's `check`, on its `058e39e` | 22 s |

`check (headless)` and `windows` went past their limit of 20 minutes again, as at M6d.1d1: the
runners' speed alone, which [`debt.md`](debt.md) follows under "CI's jobs and their limit of 20
minutes", waiting for the owner's decision.

## CI split again, 2026-11-10

The jobs of [ADR-0228](adr/0228-ci-split-again-each-job-within-20-minutes.md), tried on the
branch `ci-split` before `main`, on GitHub's hosted runners, each run green, on `8ed166e`:

| Job | Time |
|---|---|
| `check (lint)` | 10 min 34 s |
| `check (headless)` | 13 min 54 s |
| `check (windowed)` | 9 min 21 s |
| `check (game)` | 11 min 12 s |
| `windows (tests)` | 9 min 16 s |
| `windows (render)` | 6 min 52 s |
| `windows (game)` | 7 min 49 s |
| `windows-release` | 11 min 34 s |

The runners were faster than on `3a8e42e` the same night: `windows-release` took 11 min 34 s
against 15 min 52 s, the same code. `check (headless)` scaled by the slowest speed seen would take
about 19 minutes.

The same jobs on `main`, on `b7fbdd7`, each run green, the runners slower than on the branch:

| Job | Time |
|---|---|
| `check (lint)` | 9 min 57 s |
| `check (headless)` | 16 min 20 s |
| `check (windowed)` | 12 min 7 s |
| `check (game)` | 9 min 12 s |
| `windows (tests)` | 11 min 51 s |
| `windows (render)` | 5 min 46 s |
| `windows (game)` | 6 min 23 s |
| `windows-release` | 15 min 33 s |

## M6d.2a, the typed cascade

The cascade of styles as typed declarations
([ADR-0229](adr/0229-styles-typed-as-declarations.md)), on 2026-11-10, on the CPU alone, an AMD
Ryzen 7 5800X, 16 logical CPUs, Linux, release builds. *Before* is the reflected cascade, `9c2ea38`,
given `7d822db`'s `crates/uwui/look/editor.style`, trimmed of its rules' unread fields, and
`b3f4761`'s `benches/src/ui.rs`, so that both sides read the same documents and time the same
frames; *after* is the typed cascade, `b3f4761`. The two
binaries ran in turn, twice each, no build running during any run, the load average between 1.5
and 1.9.

| Benchmark | Measure, median of 15 | Before | After |
|---|---|---|---|
| `ui.tree.look` | a frame after the look's colours changed, every element styled again | 65.27, 64.82 ms | 47.19, 47.24 ms |
| `ui.tree.look` | the frames of the transition that it starts, together | 617.91, 610.11 ms | 606.67, 615.00 ms |
| `ui.tree.editor` | a frame after the theme changed, every element styled again | 19.14, 19.36 ms | 18.55, 18.36 ms |
| `ui.tree.editor` | the first frame: built, its texts and its tree laid out, drawn | 158.22, 163.53 ms | 160.11, 156.07 ms |
| `ui.tree.editor` | a frame after one value of the inspector changed | 3.51, 3.43 ms | 3.37, 3.62 ms |
| `ui.tree.editor` | a frame with nothing changed | 478.9, 484.2 µs | 480.0, 493.4 µs |
| `editor.hierarchy.100k` | a frame scrolled by a row | 273.2, 269.9 µs | 271.2, 274.3 µs |
| `editor.hierarchy.100k` | a frame that runs a search, once the typing paused | 5.37, 5.39 ms | 5.59, 5.55 ms |

`ui.tree.look`'s counters, in a debug build, as CI counts them:

| Counter | Before | After |
|---|---|---|
| allocations of reading the look's documents | 112 705 | 120 211 |
| allocations of the frame of the change | 422 651 | 393 682 |
| bytes allocated by the frame of the change | 38 057 261 | 37 007 450 |
| frames of 240 Hz of the transition | 29 | 29 |

What it says:

- Under the editor's look, the frame that styles every element again takes 27 % less time; under
  `ui.tree.editor`'s sheet of four rules, 4 %. `editor.hierarchy.100k`'s frame that runs a search
  took 3.5 % longer in both runs, outside both spreads; the cascade does not run in it, its rows
  styled before it, and the cause was not looked for. The other frames do not change beyond the
  runs' spread.
- The frame of the change allocates 7 % less, of which one allocation an element for the list of
  the rules that select it, kept from one element to the next. With less motion asked
  (`Ui::set_reduced_motion`), in a run of the typed cascade before that list was kept, the same
  frame made 243 662 allocations of 25.7 MB, against 406 156 with its transitions: these take some
  13 allocations an element, the cascade, the meaning and the drawing the rest
  ([`debt.md`](debt.md)).
- The figure of ADR-0229's context and of `43b1cb0`'s message, 537 523 allocations of 53.7 MB,
  counted the reading of the look's documents with the frame; the frame alone made 422 651.
- Reading the look's documents allocates 7 % more: each rule's part is laid over the default
  style and turned into declarations, until M6d.3 reads them with `serde`.
- The other benchmarks' counters are unchanged.

The times of compiling, measured with [`scripts/compile-times.sh`](../scripts/compile-times.sh) on
`7d822db` with the working tree's documents, on 2026-11-10, on the same machine, in the
development shell, which links with mold; runs: 7 of each change, 3 of the workspace and 1 for
shipping from nothing; load average 0.68 1.66 2.55 before, 5.54 8.43 6.35 after. Beside them,
[C1's after](#c1-after-ryzen-7-5800x), on 2026-11-08:

| Step | Median | Range | Packages compiled | C1, after |
|---|---|---|---|---|
| the workspace from nothing, `--workspace` | 63.96 s | 62.19 to 69.35 s | 276 | 61.96 s, 268 |
| then the game's editor, `-p hunnawood-editor` | 40.42 s | one run | 91 | 36.44 s, 90 |
| then the game as Play builds it, `-p hunnawood --bin hunnawood` | 2.17 s | one run | 1 | 1.83 s |
| then the examples' editor, `--example editor` | 53.55 s | one run | 130 | 49.68 s, 129 |
| then its game as Play builds it, `--workspace --example world` | 3.28 s | one run | 1 | 2.30 s |
| a function's body changed in the game, `-p hunnawood --bin hunnawood` | 1.24 s | 0.99 to 1.60 s | 1 | 1.05 s |
| a function's body changed in `slime-world`, `-p hunnawood-editor` | 1.73 s | 1.64 to 2.11 s | 8 | 1.71 s |
| then the game as Play builds it | 0.77 s | 0.76 to 1.32 s | 1 | 0.73 s |
| a function's body changed in `slime-world`, `--example editor` | 1.88 s | 1.83 to 3.49 s | 7 | 1.82 s |
| then its game as Play builds it | 0.90 s | 0.86 to 0.94 s | 1 | 0.86 s |
| a public function added to `slime-input`, `-p hunnawood-editor` | 2.20 s | 1.88 to 4.04 s | 9 | 1.96 s |
| the game for shipping from nothing, `--profile shipping -p hunnawood --no-default-features` | 81.38 s | one run | 199 | 86.70 s, 198 |

`uwui-macros` is one more package wherever `uwui` is built, the game for shipping among them, which
links it through `slime-app`. The budgets of [ADR-0215](adr/0215-the-engine-measured-against-the-market.md)
that these steps measure stay met: the editor built again after a change of an engine crate in
1.73 s, against 10 s; the whole engine from nothing in 64 s, against 3 min; a game's build for
shipping in 81 s, against 10 min; a change of a game's code, 1.24 s to build. The changes' medians
moved within their ranges; the builds from nothing took 2 to 4 s more, the workspace holding eight
packages more than at C1, `uwui-macros` one of them.

GitHub's hosted runners, on 2026-11-10, each run green, on `dc51187`:

| Job | Time |
|---|---|
| `check (lint)` | 10 min 21 s |
| `check (headless)` | 17 min 8 s |
| `check (windowed)` | 11 min 17 s |
| `check (game)` | 11 min 29 s |
| `windows (tests)` | 14 min 6 s |
| `windows (render)` | 5 min 53 s |
| `windows (game)` | 7 min 37 s |
| `windows-release` | 11 min 37 s |
| Docs' `links` | 17 s |
| uwui's `check`, on its `93a8c02` | 17 s |

`check (headless)` took 17 min 8 s, against 13 min 54 s and 16 min 20 s at ADR-0228's split: within
its limit of 20 minutes, with the least margin of the jobs. This milestone adds to it the counters of
`ui.tree.look`, about 15 s in a debug build on the owner's machine, and its tests. `windows (tests)`
ran both fixtures of styles on Windows.

## M6d.2b1, the roles and each kind's defaults

The core's roles and each kind's defaults taken from them
([ADR-0230](adr/0230-the-cores-roles-and-each-kinds-defaults.md)), on 2026-11-10, on the owner's
machine, an AMD Ryzen 7 5800X, 16 logical CPUs, Linux, release builds. *Before* is `48b2f02`, the
typed cascade of M6d.2a; *after* is the milestone's working tree. The two binaries ran in turn,
twice each, no build running; the load average fell from 6.6 to 3.6 over the runs, another
program, a session's own, taking more than a core throughout. Every frame below took about a fifth
longer than at [M6d.2a](#m6d2a-the-typed-cascade) on both sides: the comparison holds between the
two sides, not with M6d.2a's figures.

| Benchmark | Measure, median of 15 | Before | After |
|---|---|---|---|
| `ui.tree.look` | a frame after the look's colours changed, every element styled again | 57.35, 55.67 ms | 56.21, 56.44 ms |
| `ui.tree.look` | the frames of the transition that it starts, together | 809.15, 805.17 ms | 793.36, 792.41 ms |
| `ui.tree.editor` | a frame after the theme changed, every element styled again | 24.04, 25.35 ms | 24.83, 26.02 ms |
| `ui.tree.editor` | the first frame: built, its texts and its tree laid out, drawn | 199.53, 189.17 ms | 188.58, 189.27 ms |
| `ui.tree.editor` | a frame after one value of the inspector changed | 4.96, 4.33 ms | 4.84, 4.54 ms |
| `ui.tree.editor` | a frame with nothing changed | 573.5, 557.9 µs | 548.2, 554.0 µs |
| `editor.hierarchy.100k` | a frame scrolled by a row | 314.3, 309.2 µs | 318.9, 311.2 µs |
| `editor.hierarchy.100k` | a frame that runs a search, once the typing paused | 6.48, 6.23 ms | 6.58, 6.34 ms |

`ui.tree.look`'s counters, in a debug build, as CI counts them:

| Counter | Before | After |
|---|---|---|
| allocations of reading the look's documents | 120 211 | 125 017 |
| allocations of the frame of the change | 393 682 | 393 682 |
| bytes allocated by the frame of the change | 37 007 450 | 37 007 738 |
| frames of 240 Hz of the transition | 29 | 29 |

What it says:

- No frame changes beyond the runs' spread: each kind's base is made when the themes are taken,
  and an element is styled from it as it was from the one default style.
- Reading the look's documents allocates 4 % more: the five roles that the base theme names after
  the colours' themes are each read by parsing their theme again, as every value is until M6d.3.
- The frame of the change allocates 288 bytes more, its allocations unchanged: each style kept
  for the elements that share it holds the place of its base, 8 bytes more each.
- The other benchmarks' counters are unchanged.
- The fixtures of the styles changed for 1 195 of their 1 315 elements, on colours that the
  element's kind does not draw alone, compared leaf by leaf with each kind's table; the images are
  unchanged.

GitHub's hosted runners, on 2026-11-10, each run green, on `ded2bc7`:

| Job | Time |
|---|---|
| `check (lint)` | 7 min 57 s |
| `check (headless)` | 13 min 53 s |
| `check (windowed)` | 8 min 1 s |
| `check (game)` | 9 min 3 s |
| `windows (tests)` | 12 min 34 s |
| `windows (render)` | 7 min 19 s |
| `windows (game)` | 7 min 18 s |
| `windows-release` | 12 min 45 s |
| Docs' `links` | 16 s |
| uwui's `check`, on its `7c5ce7a` | 17 s |

Every job stays within its limit of 20 minutes. `check (headless)` took 13 min 53 s, 3 min less than
at M6d.2a: the runners' speed varies by minutes from run to run, and this milestone adds to it only
the roles' tests, a second or so. `windows (tests)` ran both fixtures of styles on Windows.

The times of compiling were not measured: the milestone adds no crate and no dependency, and the
machine was busy throughout, a game and other sessions' builds running.

## M6d.2b2a, an element's own style written in code

An element's own style written in code, with its states, in the mask order
([ADR-0231](adr/0231-an-elements-own-style-written-in-code.md)), on 2026-11-10, on the owner's
machine, an AMD Ryzen 7 5800X, 16 logical CPUs, Linux, release builds. *Before* is `8408b1e`, the
code of M6d.2b1, with this milestone's new counter added to its benchmark; *after* is `a3695b4`. The
two binaries ran in turn, twice each, no build running; the load average rose from 2.3 to 4.9 over
the runs, a program of the owner's taking more than half a core throughout, and a session's build
started right after them. The runs of one binary differ by up to a quarter: no frame below differs
between the sides beyond that spread.

| Benchmark | Measure, median of 15 | Before | After |
|---|---|---|---|
| `ui.tree.look` | a frame after the look's colours changed, every element styled again | 56.72, 53.49 ms | 56.67, 52.89 ms |
| `ui.tree.look` | the frames of the transition that it starts, together | 909.23, 733.18 ms | 743.92, 849.35 ms |
| `ui.tree.editor` | a frame after the theme changed, every element styled again | 28.05, 28.56 ms | 27.79, 23.15 ms |
| `ui.tree.editor` | the first frame: built, its texts and its tree laid out, drawn | 191.32, 193.53 ms | 181.53, 169.36 ms |
| `ui.tree.editor` | a frame after one value of the inspector changed | 4.78, 5.10 ms | 7.56, 4.68 ms |
| `ui.tree.editor` | a frame with nothing changed | 526.1, 528.3 µs | 643.8, 545.9 µs |

The counters, in a debug build, as CI counts them:

| Benchmark | Counter | Before | After |
|---|---|---|---|
| `ui.tree.editor` | allocations of a frame after one value of the inspector changed | 28 714 | 32 163 |
| `ui.tree.editor` | bytes allocated by a frame after one value of the inspector changed | 6 384 534 | 7 157 318 |
| `ui.tree.look` | allocations of reading the look's documents | 125 017 | 127 736 |
| `ui.tree.look` | allocations of the frame of the change | 393 682 | 393 706 |
| `ui.tree.look` | bytes allocated by the frame of the change | 37 007 738 | 36 988 023 |
| `ui.tree.look` | frames of 240 Hz of the transition | 29 | 29 |

What it says:

- A frame that builds again allocates 12 % more, in allocations and in bytes: each element whose
  code writes a style holds its declarations in a list of its own, where it held a whole `Style`
  in place. The builder's list is taken whole by the element, not copied: copying it cost another
  10 %.
- Reading the look's documents allocates 188 more for the arrays, each now one declaration for
  each element, and 2 531 more for the four rules that show the keyboard's focus.
- The frame after a change of colours allocates 24 more and 20 kB less: each style kept for the
  elements that share it holds their own style's declarations, not a whole `Style`.
- The other benchmarks' counters are unchanged; the fixtures of the styles and the images too.
- The times of compiling were not measured: the machine was busy with other sessions' builds
  before and after the runs.

GitHub's hosted runners, on 2026-11-10, each run green, on `b749c3e`:

| Job | Time |
|---|---|
| `check (lint)` | 6 min 39 s |
| `check (headless)` | 12 min 28 s |
| `check (windowed)` | 11 min 17 s |
| `check (game)` | 9 min 31 s |
| `windows (tests)` | 10 min 1 s |
| `windows (render)` | 6 min 46 s |
| `windows (game)` | 7 min 37 s |
| `windows-release` | 15 min 13 s |
| Docs' `links` | 20 s |
| uwui's `check`, on its `d92475c` | 21 s |

Every job stays within its limit of 20 minutes. `windows (tests)` ran both fixtures of styles on
Windows, and the tests of the own style, the place of a call among them.

## M6d.3a, uwui's reader of KDL

uwui's reader of KDL 2.0, `uwui-kdl`
([ADR-0241](adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)), on 2026-11-11, on
the CPU alone, an AMD Ryzen 7 5800X, 16 logical CPUs, Linux, release builds. `ui.kdl.read` reads two
documents of rules and themes' values in ADR-0241's notation, run twice:

| Measure, median of 30 | Run 1 | Run 2 |
|---|---|---|
| a document of the editor's look's size, 29 206 bytes, read | 279.7 µs | 262.4 µs |
| a document of 4 194 497 bytes, read | 46.33 ms | 47.24 ms |

Its counters, the same in a debug build, as CI counts them:

| Counter | Value |
|---|---|
| allocations of reading the look document | 28 |
| bytes taken reading the look document | 401 136 |
| bytes that the look document holds | 188 416 |
| allocations of reading the large document | 49 |
| bytes taken reading the large document | 51 379 952 |
| bytes that the large document holds | 24 117 248 |
| allocations of reading the most nodes document, 2^20 nodes `a;` | 20 |
| bytes taken reading the most nodes document | 150 994 752 |
| bytes that the most nodes document holds | 50 331 648 |

What it says:

- The reader reads some 90 to 110 MB a second, its time growing with the text: 143 times the text
  takes 170 times as long.
- A document of rules and values holds some six bytes for each byte of its text once read, in a
  few dozen allocations whose number grows with the logarithm of the text's size; as its arrays
  grow, reading allocates about twice what the document holds in all, the old arrays given back.
  The most nodes that the default limits allow, 2^20 in 2 MiB, hold 50 MB, 48 bytes a node, and
  reading them allocates 151 MB in all: the bound of memory that the limits set, which the
  reader's rustdoc states. Its time was not taken.
- Reading the look's documents through reflection today takes 127 736 allocations, but holds
  more: the typed declarations, which M6d.3b reads from this tree.

The frames of `ui.tree.look` and `ui.tree.editor` had not been timed since M6d.2b2b1. *Before* is
`13ba018`, the code of M6d.2b2a; *after* is `185ed10`, whose uwui is M6d.2b2b3's. The two binaries
ran in turn, twice each, on lavapipe's settings; another session's `cargo` ran during some runs,
the load average between 2.2 and 2.6:

| Benchmark | Measure, median of 15 | Before | After |
|---|---|---|---|
| `ui.tree.look` | a frame after the look's colours changed, every element styled again | 53.34, 52.49 ms | 46.42, 44.33 ms |
| `ui.tree.look` | the frames of the transition that it starts, together | 800.35, 765.11 ms | 571.76, 499.99 ms |
| `ui.tree.look` | a frame after the size of text changed, every text laid out again | not measured | 155.73, 154.05 ms |
| `ui.tree.editor` | a frame after the theme changed, every element styled again | 22.20, 22.54 ms | 20.10, 16.97 ms |
| `ui.tree.editor` | the first frame: built, its texts and its tree laid out, drawn | 160.32, 164.77 ms | 149.95, 145.90 ms |
| `ui.tree.editor` | a frame after one value of the inspector changed | 3.94, 4.18 ms | 3.89, 3.36 ms |
| `ui.tree.editor` | a frame with nothing changed | 517.6, 549.1 µs | 522.3, 498.7 µs |

- The frames of a transition after a change of colours take 30 % less time, which the styles
  shared by elements that move alike give (M6d.2b2b1); the frame of the change 14 % less.
- The other frames change within the runs' spread, which the load widens.
- The times of compiling `uwui` and `uwui-kdl` were not measured: the machine was not at rest.

GitHub's hosted runners, on 2026-11-11, each run green, on `0e675a0`:

| Job | Time |
|---|---|
| `check (lint)` | 10 min 18 s |
| `check (headless)` | 16 min 48 s |
| `check (windowed)` | 9 min 11 s |
| `check (game)` | 9 min 46 s |
| `windows (tests)` | 14 min 13 s |
| `windows (render)` | 7 min 41 s |
| `windows (game)` | 5 min 55 s |
| `windows-release` | 14 min 37 s |
| Docs' `links` | 19 s |

Every job stays within its limit of 20 minutes. `check (headless)` took as long as on `2ef0262`,
its margin 3 min 12 s; `windows-release` 1 min 51 s less, its margin 5 min 23 s. The new crate
and its 338 tests of KDL add some seconds to the jobs that test the workspace.

## M6d.3c2, docks' layouts in KDL

The editor's default layout read in KDL by `uwui::dock`'s own reader, in place of the engine's
reflection reading TOML ([ADR-0241](adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)),
on 2026-11-12. `ui.tree.look` reads it strictly beside the look's documents and the tables of
shortcuts, as the editor does at its start.

Reading the editor's default layout allocates 50 times in a debug build, as CI counts it; every
other counter is unchanged. Before, the editor read its default in TOML through reflection: a test
not kept in the repository, which read that file through `slime_serialize::read_as` and fitted its
panels under an allocator that counts, counted 1 872 allocations in a release build, 1 873 in a
debug one; the same test counted 76 for reading the KDL and fitting it.

Their times were not measured: other sessions' builds and the desktop's indexing kept the load
average between 2 and 9 throughout ([`debt.md`](debt.md)). Nor was the cooker's time of compiling
again, which M6d.3c1 left; the milestone did not change the cooker.

Every finite `f32`, 4 278 190 080 values, was written by Rust's `Display` and read back by
`str::parse::<f32>` to the same bits, each text a decimal of KDL; read through an `f64`, two values,
±7.038531e-26, rounded twice to the next `f32`. A sweep of 156 s on 12 threads, run once; the tests
of `uwui-kdl` check a sample and these two values (`write_f32`, `Number::to_f32`).

GitHub's hosted runners, on 2026-11-12, each run green, on `0ccb4fa`; `Compositors` did not run, its
paths untouched:

| Job | Time |
|---|---|
| `check (lint)` | 6 min 39 s |
| `check (headless)` | 11 min 55 s |
| `check (windowed)` | 11 min 3 s |
| `check (game)` | 15 min 12 s |
| `windows (tests)` | 15 min 45 s |
| `windows (render)` | 7 min 6 s |
| `windows (game)` | 8 min 40 s |
| `windows-release` | 16 min 12 s |
| Docs' `links` | 17 s |

Every job stays within its limit of 20 minutes; the closest, `windows-release`, has 3 min 48 s to
spare.

## M6d.4b1a, the canvas moved into uwui-render

The canvas moved out of `slime-render` into `uwui-render`, still on `slime-gfx`'s handles, and
drawn through `slime-app`'s `InterfaceRenderer`
([ADR-0244](adr/0244-the-engine-lends-its-device-through-a-guarded-door.md)).

### `render.canvas` and `ui.draw.menu` before and after

At 3840 × 2160 on the RX 6900 XT (RADV, Mesa 26.2.6), the GPU's clock fixed at `high`, offscreen,
release builds, three runs each, on 2026-11-12: before on `fbbfdf8`, after on `58199e1`, the
change rebased since without a change of code. Each figure is a median of 30 frames; the range
is that of the three runs. The GPU time is the canvas's pass, as `render.canvas` has always
measured it
([ADR-0245](adr/0245-the-interfaces-gpu-budget-counts-what-it-adds-to-the-frame.md) keeps it for
M6d.4b1).

| Screen | GPU before | GPU after | CPU before | CPU after |
|---|---|---|---|---|
| `render.canvas`, the editor's screen | 291.8–292.1 µs | 291.2–291.3 µs | 624.7–730.4 µs | 555.3–568.1 µs |
| `render.canvas`, the game's menu | 233.9–234.1 µs | 233.6 µs | 198.9–216.4 µs | 189.6–195.6 µs |
| `ui.draw.menu` | 207.7–208.0 µs | 207.6–207.9 µs | 365.2–396.5 µs | 339.3–372.5 µs |
| `ui.draw.menu`, the first draw | — | — | 2.13–2.83 ms | 2.00–2.09 ms |

What they say:

- The GPU's work did not change: the same items, draws and bytes, the same pass.
- The CPU did not grow: the bridge's conversion of the images and the check of their indices are
  lost in the runs' spread, and the machine was quieter for the runs after.
- Every screen keeps the interface's 0.5 ms. The CPU's rows are named after the call that they
  time: `InterfaceRenderer::draw_canvas` and `InterfaceRenderer::draw`, where they named
  `Renderer::draw_canvas` and `UiRenderer::draw`.
- The counters of CI are unchanged: items and draws are the same, and `counters.txt` did not
  change. M6d.4b1a touched no phase of uwui's that it counts, so it adds no counter.

### What a program on uwui compiles now

[`scripts/uwui-program.sh`](../scripts/uwui-program.sh), as in
[M6d.0a](#what-a-program-on-uwui-compiles): the `widgets` example in a package of its own, its
dependencies listed anew, since it no longer names `slime-render` nor `uwui-render`. Times on a
machine that other sessions loaded during the runs, from 1.5 to 16: only the counts and the sizes
compare.

| | `development`, `accessibility` | No feature |
|---|---|---|
| Crates compiled, the engine's among them | 271, 15 | 207, 14 |
| `slime-render`, `slime-world`, `slime-editor` | not, compiled, not | not, compiled, not |
| Debug build, from nothing / after a change of its code | 68.0 s / 0.9 s | 49.0 s / 0.8 s |
| Release build, from nothing / after a change of its code | 56.0 s / 2.2 s | 41.6 s / 2.1 s |
| Release binary / stripped | 31.4 MB / 23.8 MB | 24.6 MB / 18.8 MB |

A program on uwui no longer compiles the renderer, which ADR-0180 asked for; it still compiles
the world, through `slime-app`, which M6d.5 and M6d.7 follow. Stripped and without the development
channel, its binary is 1.2 MB within uwui's budget of 20 MB.

### The canvas's cost, against the renderer's

`scripts/canvas-cost.sh --runs 1` ran on lavapipe, from its new home, `benches/tests/canvas_cost.rs`:
the canvas built in 4.0 ms and drawn first in 13.2 ms, the renderer beside it in 135.2 ms and
146.0 ms. One run on the CPU's driver checks the probe; its figures on the GPU wait for the next
series that needs them. The dev overlay now builds its canvas at the first frame that shows a
panel, where the view's renderer built it before: those 4 ms on lavapipe move there.

### CI

GitHub's hosted runners, on 2026-11-12, each run green, on `68af09b`; `Compositors` ran, since
`slime-app` changed:

| Job | Time |
|---|---|
| `check (lint)` | 6 min 45 s |
| `check (headless)` | 11 min 57 s |
| `check (windowed)` | 7 min 58 s |
| `check (game)` | 12 min 26 s |
| `windows (tests)` | 14 min 50 s |
| `windows (render)` | 7 min 4 s |
| `windows (game)` | 7 min 52 s |
| `windows-release` | 9 min 53 s |
| `compositors`, the longest, KWin | 14 min 6 s |
| Docs' `links` | 20 s |

Every job stays within its limit of 20 minutes; the closest, `windows (tests)`, has 5 min 10 s to
spare.

## M6d.4b1b, the canvas on wgpu

`uwui-render` draws with a wgpu device, its queue and its host's encoder, lent through
`slime-gfx`'s guarded door by `slime-app`'s bridge
([ADR-0244](adr/0244-the-engine-lends-its-device-through-a-guarded-door.md)).

### `render.canvas` and `ui.draw.menu` before and after

At 3840 × 2160 on the RX 6900 XT (RADV, Mesa 26.2.6), the GPU's clock fixed at `high`, offscreen,
release builds, on 2026-11-12: three runs of each, before on `bb0adb5`, M6d.4b1a's code, and after
on `37b17fb`, in turn, run after run, in the same minute, the load average under 1.5 at their
start. Each figure is a median of 30 frames; the range is that of the three runs. The GPU time is
the canvas's pass, as M6d.4b1a measured it
([ADR-0245](adr/0245-the-interfaces-gpu-budget-counts-what-it-adds-to-the-frame.md) keeps it for
M6d.4b1).

| Screen | GPU before | GPU after | CPU before | CPU after |
|---|---|---|---|---|
| `render.canvas`, the editor's screen | 291.9 µs | 291.7–291.8 µs | 648.6–969.9 µs | 364.1–407.7 µs |
| `render.canvas`, the game's menu | 233.8–234.0 µs | 234.1 µs | 219.1–254.3 µs | 145.5–168.8 µs |
| `ui.draw.menu` | 207.7–207.8 µs | 207.7–208.0 µs | 364.6–375.1 µs | 330.2–396.7 µs |
| `ui.draw.menu`, the first draw | — | — | 2.41–2.55 ms | 2.10–2.19 ms |

What they say:

- The GPU's work did not change: the same items, draws and pass. The canvas's images are the same,
  byte for byte, in the `interface` example's five images, dark and light, mirrored, at twice the
  scale and with its dialog, drawn on lavapipe before and after.
- The CPU of drawing a canvas fell by some 40 %. Its first form on wgpu cost more than M6d.4b1a,
  the editor's screen 697–791 µs against 545–704 µs in five runs in turn: timed inside the draw,
  the door, its scopes and its probe took about 65 µs, and writing the 19 377 items' bytes
  381 µs before and 513 µs after, from the same source. The writer appended each item's 36 values
  one by one to a growing vector, as fast as the compiler inlined that growth, which the crates'
  new shape changed. Each item is now laid out in place, then appended at once.
- `ui.draw.menu` keeps its CPU within the runs' spread: its 2 645 items weigh less beside its
  text's work. Every screen keeps the interface's 0.5 ms.
- The counters of CI are unchanged: items and draws are the same, and `counters.txt` did not
  change. M6d.4b1b touched no phase of uwui's that it counts, so it adds no counter.

### CI

GitHub's hosted runners, on 2026-11-12, each run green, on `37b17fb`; `Compositors` ran, since
`slime-app` changed:

| Job | Time |
|---|---|
| `check (lint)` | 9 min 59 s |
| `check (headless)` | 13 min 54 s |
| `check (windowed)` | 11 min 11 s |
| `check (game)` | 7 min 49 s |
| `windows (tests)` | 13 min 46 s |
| `windows (render)` | 6 min 26 s |
| `windows (game)` | 6 min 9 s |
| `windows-release` | 15 min 45 s |
| `compositors`, the longest, sway | 15 min 31 s |
| Docs' `links` | 18 s |

Every job stays within its limit of 20 minutes; the closest, `windows-release`, has 4 min 15 s to
spare. `check (lint)` and `check (headless)` took 3 and 2 minutes more than at M6d.4b1a, all their
jobs starting together: the door's check adds a `cargo tree` and a `git grep`, seconds.

## M6d.4b2, the canvas's layout chosen

The canvas reads its items by their index, each field when its kind needs it, from a storage
buffer or from a texture as the device allows
([ADR-0243](adr/0243-the-canvas-reads-its-items-by-index-from-a-buffer-or-a-texture.md)), and is
tested on Mesa's OpenGL ES
([ADR-0247](adr/0247-the-canvas-tested-on-opengl-es-through-uwui-renders-feature.md)).

### `render.canvas` and `ui.draw.menu` before and after

At 3840 × 2160 on the RX 6900 XT (RADV, Mesa 26.2.6), the GPU's clock fixed at `high`, offscreen,
release builds, on 2026-11-12: three runs of each, before on `38bc80b`, M6d.4b1b's code, and after
on `5291eb1`, in turn, run after run, in the same minute, the load average under 1.5 at their
start. Each figure is a median of 30 frames; the range is that of the three runs. The GPU's figure
is the canvas's pass alone, its GPU idle before it, as M6d.4b1b measured it. The device has
storage buffers: the canvas reads from one.

| Screen | GPU before | GPU after | CPU before | CPU after |
|---|---|---|---|---|
| `render.canvas`, the editor's screen | 291.2–291.4 µs | 258.6–258.8 µs | 368.7–408.9 µs | 345.2–378.3 µs |
| `render.canvas`, the game's menu | 233.6–233.8 µs | 221.4 µs | 155.9–163.9 µs | 137.6–160.9 µs |
| `ui.draw.menu` | 207.7–207.8 µs | 198.6–198.8 µs | 329.9–331.3 µs | 332.2–383.4 µs |

What the interface adds to a frame over a view, the examples' sky with its bloom and tone mapping,
as [ADR-0245](adr/0245-the-interfaces-gpu-budget-counts-what-it-adds-to-the-frame.md) counts the
budget, the same runs, the median of 30 pairs of frames; its passes within that frame beside it,
one pass, which is its own span:

| Screen | Added to the frame | Its passes within the frame |
|---|---|---|
| `render.canvas`, the editor's screen | 260.2–260.6 µs | 258.8–258.9 µs |
| `render.canvas`, the game's menu | 221.9–222.6 µs | 221.3–221.4 µs |
| `ui.draw.menu` | 199.0–199.8 µs | 199.0 µs |

At the smaller tiers, what the interface adds, in the same runs: the editor's screen 44.6–44.7 µs
at 1280 × 800 and 76.3–76.5 µs at 1920 × 1080; the game's menu 31.0–31.1 µs and 60.0–60.3 µs;
`ui.draw.menu` 28.2–28.3 µs and 53.9–54.2 µs.

What they say:

- Reading each field when its kind needs it takes 11 % off the editor's screen on the GPU, 5 % off
  the game's menu and 4 % off `ui.draw.menu`: a fragment no longer loads the item's nine words
  whole. Its images are the same: the `interface`
  example's five images, dark and light, mirrored, at twice the scale and with its dialog, identical
  byte for byte on lavapipe before and after.
- The CPU stays within the runs' spread: the items' bytes are written as before, 32 bytes of
  transforms after them.
- What the interface adds to a frame is its passes' time and 1 to 2 µs: one boundary between
  the view's last pass and the canvas's, the GPU's wait there. Every screen keeps the interface's
  0.5 ms, counted so, at 3840 × 2160; at 1280 × 800 the editor's screen adds 44.6 µs, within the
  handheld tier's 50 µs with one pass.
- The texture's store was not timed on the RX 6900 XT, which has storage buffers; M6d.4a's
  prototype measured it 13 to 20 % slower than S2. On lavapipe, at WebGL2's limits, it draws the
  bytes of the buffer, and on Mesa's OpenGL ES 3.0 those of lavapipe within 1/255 on 106 channels
  of 65,536.
- The counters of CI gain `passes of the interface`, 1 on each screen and tier.

### CI

GitHub's hosted runners, on 2026-11-12, each run green, on `5291eb1`; `Compositors` ran, since
`flake.nix` changed:

| Job | Time |
|---|---|
| `check (lint)` | 10 min 10 s |
| `check (headless)` | 11 min 1 s |
| `check (windowed)` | 7 min 20 s |
| `check (game)` | 12 min 58 s |
| `check (gles)` | 3 min 17 s |
| `windows (tests)` | 15 min 38 s |
| `windows (render)` | 7 min 16 s |
| `windows (game)` | 5 min 30 s |
| `windows-release` | 15 min 43 s |
| `compositors`, the longest, Mutter | 14 min 24 s |
| Docs' `links` | 19 s |

Every job stays within its limit of 20 minutes; the closest, `windows-release` and
`windows (tests)`, have some 4 minutes 20 s to spare. The new job, `check (gles)`, builds
`uwui-render`'s tree alone with wgpu's GL backend and runs its tests twice, in 3 min 17 s.
`check (game)` took 5 min 9 s more than at M6d.4b1b. Its steps whose code did not change took 1.5
to 2 times as long, the zone cooked and its tests 5 min 5 s against 3 min 22 s, the game in
Weston 4 min 1 s against 2 min 3 s: the runner was slower, as the runners' speed varies from one
day to the next ([`debt.md`](debt.md)). The counters' step took 2 min 18 s against 1 min 2 s, which
also counts the frames that `render.canvas` and `ui.draw.menu` now draw over a view.

## M6d.3c3, reflection out of uwui

uwui without `slime-reflect` and `slime-serialize`, and `LinearColor` described by `slime-reflect`
([ADR-0241](adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)), on 2026-11-12, an
AMD Ryzen 7 5800X, 16 logical CPUs, Linux, the machine at rest: the load average under 0.8 and no
other `cargo` or `rustc` at the start of each series.

The time of compiling `uwui` and `slime-cook` again, in debug, after a public function is added to
the crate's root, `cargo build -p <crate>`, five runs each, before and after in turn. *Before* is
`38bc80b`, in a worktree with a target directory of its own; *after* is `4c57a59`. A first series
of the cooker, whose first run built its dependencies again with other features, was set aside:

| Crate | Before, median and range | After, median and range |
|---|---|---|
| `uwui` | 0.93 s, 0.81 to 1.05 | 0.85 s, 0.82 to 0.93 |
| `slime-cook` | 0.57 s, 0.56 to 0.58 | 0.57 s, 0.57 to 0.68 |

- `uwui` compiles again some 9 % faster without its derives of reflection; the cooker, which the
  milestone did not change, as fast.

At `10b86bb`, the machine at rest, the times that M6d.3a and M6d.3b2 left unmeasured at rest: in
debug, `uwui-kdl` again after the same change, five runs, and `uwui-kdl` and `uwui` from nothing,
`cargo build -p <crate>` into an empty target directory, the dependencies downloaded, three runs;
the load average rose to 7.9 with the builds of `uwui`'s own dependencies:

| Build | Median | Range |
|---|---|---|
| `uwui-kdl` again | 0.16 s | 0.15 to 0.18 |
| `uwui-kdl` from nothing | 0.34 s | 0.34 to 0.34 |
| `uwui` from nothing, its dependencies with it | 19.89 s | 19.51 to 21.03 |

The time of reading the editor's default layout and fitting it to the editor's panels, left by
M6d.3c2: through reflection reading its TOML at `8a1d064`, and through `uwui::dock`'s reader of
its KDL at `38bc80b`, whose reader `4c57a59` keeps unchanged. A test not kept in the repository
read it 1 000 times, seven times, after 200 reads to warm up, in release builds, three runs each,
in turn:

| Measure, median of seven batches | Before, TOML through reflection | After, KDL |
|---|---|---|
| the editor's default layout read and fitted | 75.9, 76.2, 77.7 µs | 9.4, 9.9, 9.4 µs |

- Reading the editor's default layout takes some 88 % less time, as its allocations fell from
  1 872 to 76 ([M6d.3c2](#m6d3c2-docks-layouts-in-kdl)).

The fixtures of verdicts compare a colour's numbers within 1e-5: on GitHub's Windows runner, the
largest difference was 3.84e-7 in the themes' fixture and 3e-8 in the sheets', on `5d45d2d`; on
Linux, none.

GitHub's hosted runners, on 2026-11-12, each run green, on `5d45d2d`; `Compositors` and Docs did
not run, their paths untouched:

| Job | Time |
|---|---|
| `check (lint)` | 9 min 51 s |
| `check (headless)` | 17 min 10 s |
| `check (windowed)` | 8 min 58 s |
| `check (game)` | 10 min 21 s |
| `windows (tests)` | 15 min 24 s |
| `windows (render)` | 6 min 32 s |
| `windows (game)` | 7 min 7 s |
| `windows-release` | 12 min 7 s |

Every job stays within its limit of 20 minutes; the closest, `check (headless)`, has 2 min 50 s to
spare. It took 3 min 16 s more than at M6d.4b1b: each of its phases ran some 25 % slower, the
build 6 min 28 s from 5 min 2 s, the tests 6 min 41 s from 5 min 33 s, a check of a few seconds as
slow, the runner slower rather than the milestone heavier; on `061fd88`, its first run after
`Cargo.lock` changed, 17 min 19 s.

## M6d.3c1, shortcuts in KDL

The editor's tables of shortcuts read in KDL by uwui's own reader, in place of the engine's
reflection ([ADR-0241](adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)), on
2026-11-12. `ui.tree.look` reads them, its own and Blender's way laid over it, beside the look's
documents, as the editor does at its start.

Reading the editor's two tables allocates 324 times, from 1 117, in a debug build, as CI counts
them; reading the look's documents is unchanged, 3 376, and so is every other counter. Of the 324,
the table that finds two chords of the same keys, on each platform, takes some 120: it keeps the
reading linear in the table, where comparing each chord with every other took 205 allocations and
time that grew with the square of a scope's size.

On 2026-11-12, an AMD Ryzen 7 5800X, 16 logical CPUs, Linux, release builds on lavapipe's settings.
*Before* is `96bd624`, its tables in TOML read through reflection, the benchmark's reading of them
added alone; *after* is the milestone's code before its closing review, which then made the search
for chords given twice linear: these times were not taken again, the machine busy. The two binaries ran in turn, twice each, the load
average 0.92 at the start and 3.48 at the end, another session's build starting during the second
round:

| Measure, median of 15 | Before | After |
|---|---|---|
| the editor's tables of shortcuts read, its own and Blender's way, by `Ui::set_shortcuts` | 94.4, 97.8 µs | 44.8, 46.2 µs |
| the look's documents read, its sheet and its themes, by `Ui::set_style` | 685.2, 824.5 µs | 694.8, 756.9 µs |

- Reading the tables took half the time: what reflection allocated for each chord, its path and its
  value, is gone.
- The editor's documents read at its start, the look and the tables, take some 0.75 ms; the look's
  reading changes within the runs' spread.

The cooker's time of compiling again was not measured: other sessions' benchmarks kept the machine
busy ([`debt.md`](debt.md)). Its dependencies did not change; it no longer encodes a table of
shortcuts into the binary form.

GitHub's hosted runners, on 2026-11-12, each run green, on `44bf0f8`; `Compositors` did not run, its
paths untouched:

| Job | Time |
|---|---|
| `check (lint)` | 9 min 51 s |
| `check (headless)` | 11 min 42 s |
| `check (windowed)` | 11 min 47 s |
| `check (game)` | 15 min 2 s |
| `windows (tests)` | 14 min 39 s |
| `windows (render)` | 7 min 42 s |
| `windows (game)` | 10 min 50 s |
| `windows-release` | 15 min 48 s |
| Docs' `links` | 17 s |

Every job stays within its limit of 20 minutes; the closest, `windows-release`, has 4 min 12 s to
spare.

## M6d.3b2, style sheets in KDL

The look's style sheet read in KDL by the code that `uwui-macros` derives, as its themes are
([ADR-0241](adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)), on 2026-11-11.

Reading the look's documents allocates 3 376 times, from 95 970, in a debug build, as CI counts
them; every other counter is unchanged. What reflection allocated for each value of each rule is
gone: a rule's name is borrowed from the text, its declarations made as they are read.

On 2026-11-12, an AMD Ryzen 7 5800X, 16 logical CPUs, Linux, release builds on lavapipe's settings.
*Before* is `cb45105`, its sheet read through reflection; *after* is `cbc1390`. The two binaries ran
in turn, twice each, the load average 0.88 at the start:

| Measure, median of 15 | Before | After |
|---|---|---|
| the look's documents read, its sheet and its themes, by `Ui::set_style` | 4.87, 5.05 ms | 0.82, 0.79 ms |
| a frame after the look's colours changed, every element styled again | 47.91, 48.66 ms | 47.90, 48.39 ms |
| the frames of the transition that it starts, together | 536.26, 532.16 ms | 555.12, 531.92 ms |
| a frame after the size of text changed, every text laid out again | 142.26, 155.31 ms | 149.90, 159.77 ms |

- Reading the look's documents takes 84 % less time: 0.8 ms, from 8.2 ms before M6d.3b1.
- The frames change within the runs' spread.

The time of compiling `uwui-kdl` and `uwui` again, in debug, after a public function is added to
each crate's root, `cargo build -p <crate>`, five runs each, before and after in turn; the load
average rose from 1.8 to 4.3 as they ran, the figures indicative:

| Crate | Before, median and range | After, median and range |
|---|---|---|
| `uwui-kdl` | 0.18 s, 0.17 to 0.18 | 0.19 s, 0.18 to 0.20 |
| `uwui` | 0.89 s, 0.85 to 0.93 | 0.95 s, 0.94 to 0.98 |

- `uwui` compiles again 7 % slower, the readers that its derives generate added; both stay under a
  second.

GitHub's hosted runners, on 2026-11-12, each run green, on `cbc1390`, the first run after the
toolchain gained the web's target, which the caches' keys hold:

| Job | Time |
|---|---|
| `check (lint)` | 7 min 28 s |
| `check (headless)` | 13 min 50 s |
| `check (windowed)` | 11 min 23 s |
| `check (game)` | 10 min 50 s |
| `windows (tests)` | 15 min 11 s |
| `windows (render)` | 7 min 30 s |
| `windows (game)` | 11 min 27 s |
| `windows-release` | 15 min 56 s |
| `compositors (sway)` | 11 min 56 s |
| `compositors (weston)` | 13 min 37 s |
| `compositors (x11)` | 14 min 30 s |
| `compositors (mutter)` | 16 min 18 s |
| `compositors (kwin)` | 16 min 58 s |
| Docs' `links` | 20 s |

Every job stays within its limit of 20 minutes; the closest, `compositors (kwin)`, has 3 min 2 s to
spare, and `check (headless)` 6 min 10 s, 3 min 46 s faster than on `58082b9`.

uwui's weight in a build for the web, by a probe outside the repository that lays out and draws a
label and a button, its variant *read* reading a sheet and a theme that the host gives as text:
built with the project's toolchain, 1.99.0, for `wasm32-unknown-unknown` in release, its size
optimised (`opt-level = "z"`, fat LTO, one codegen unit, `panic = "abort"`, stripped), not passed
through `wasm-opt`, and compressed by brotli 1.2.0 at its quality 11. *Before* is `cb45105`, the
sheet read through reflection; *after* is `8f045a2`:

| Module | Raw | Brotli |
|---|---|---|
| before, nothing read | 4 540 078 B | 2 371 459 B |
| before, the documents read | 4 826 665 B | 2 455 315 B |
| after, nothing read | 4 428 749 B | 2 349 389 B |
| after, the documents read | 4 560 599 B | 2 382 996 B |
| after, `parley` without `complex-scripts`, nothing read | 1 374 559 B | 417 564 B |
| after, `parley` without `complex-scripts`, the documents read | 1 509 043 B | 454 679 B |

- Reading the documents weighs 33 607 B compressed, from 83 856 B: 37 115 B in a module without
  `parley`'s dictionaries, which compress less well beside it. uwui's core is 22 kB lighter, its
  interface keeping no registry of reflection.
- uwui's core weighs 2.35 MB compressed, against the provisional budget of 1 MB
  ([ADR-0181](adr/0181-uwuis-scope.md)); 1.93 MB of it are ICU4X's dictionaries of the words of
  Chinese and Japanese, Burmese, Khmer, Lao and Thai, which `parley`'s feature `complex-scripts`
  brings; without them, 0.42 MB. The owner kept it as debt ([`debt.md`](debt.md)).

## M6d.3b1, themes in KDL

The look's themes read in KDL by uwui's own reader, the look's style sheet still through
reflection ([ADR-0241](adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)), on
2026-11-11, an AMD Ryzen 7 5800X, 16 logical CPUs, Linux, release builds on lavapipe's settings.
*Before* is `2f4dbcb`, its themes read through reflection, built with `ui.tree.look`'s new series;
*after* is `58082b9`. The two binaries ran in turn, twice each, the load average below 1:

| Measure, median of 15 | Before | After |
|---|---|---|
| the look's documents read, its sheet and its themes, by `Ui::set_style` | 8.22, 8.18 ms | 4.30, 4.36 ms |
| a frame after the look's colours changed, every element styled again | 44.13, 42.91 ms | 40.63, 41.52 ms |
| the frames of the transition that it starts, together | 474.76, 440.54 ms | 434.15, 438.17 ms |
| a frame after the size of text changed, every text laid out again | 135.78, 132.22 ms | 130.61, 128.73 ms |

Reading the look's documents allocates 95 970 times, from 127 736, in a debug build, as CI counts
them; every other counter is unchanged.

- Reading the look's documents takes 47 % less time and allocates 25 % less: a theme is read once,
  where reflection parsed it again for each value. What remains is the sheet's, read through
  reflection until M6d.3b2.
- The frames change within the runs' spread.
- The times of compiling `uwui` and `uwui-kdl` were not measured: another session's `cargo` ran,
  the load average between 3.3 and 3.8.

GitHub's hosted runners, on 2026-11-11, each run green, on `58082b9`, `windows-release` run again
after GitHub refused its artefact five times:

| Job | Time |
|---|---|
| `check (lint)` | 9 min 40 s |
| `check (headless)` | 17 min 36 s |
| `check (windowed)` | 11 min 38 s |
| `check (game)` | 15 min 23 s |
| `windows (tests)` | 12 min 27 s |
| `windows (render)` | 7 min 54 s |
| `windows (game)` | 8 min 15 s |
| `windows-release` | 14 min 56 s |
| Docs' `links` | 21 s |

Every job stays within its limit of 20 minutes. `check (headless)` took 48 s more than on `0e675a0`,
its margin 2 min 24 s; on `32b3f0b`, before the review's changes, it took 17 min 30 s.

## M6d.4a, the canvas's prototype

The canvas's *layouts*, the ways it hands its items to its shaders, drawn by the prototype of
[ADR-0243](adr/0243-the-canvas-reads-its-items-by-index-from-a-buffer-or-a-texture.md), at
[`744dce3`](https://github.com/HoneySlatt/SlimeEngine/tree/744dce3373481e0ee054e665165a0fc5b4fc4faf/crates/canvas-prototype)
on the branch `m6d4a-prototype`, on 2026-11-12. S is today's storage buffer, each fragment reading
its whole item; S2 the same buffer, fields read one by one; T a texture read by index, fields one
by one; I instances, one format for each kind. 30 frames of each layout after 10 of warm-up, the
layouts in turn frame by frame, each frame waited for; medians, in µs. The *passes* are the GPU
time of the frame's render passes, summed, as `render.canvas` times the canvas; the *span*, in
brackets, runs from the frame's first copy of its items to the end of its last pass, the gaps
between passes included. The bytes are those written each frame.

On the RX 6900 XT, its clocks held at their highest by the owner, the machine at a load of 1.5,
through radv for Vulkan and radeonsi for OpenGL ES 3.0, Mesa 26.2.6; the two screens of
`render.canvas`, on Vulkan:

| Screen, size | S | S2 | I | T |
|---|---|---|---|---|
| Editor, 1280 × 800 | 46.8 (58.3) | 44.1 (55.5) | 47.5 (57.1) | 50.0 (62.1) |
| Editor, 1920 × 1080 | 80.0 (91.4) | 75.7 (87.0) | 78.2 (87.8) | 87.5 (99.5) |
| Editor, 3840 × 2160 | 291.6 (303.1) | 258.6 (270.1) | 267.8 (277.4) | 307.9 (320.0) |
| Game's menu, 1280 × 800 | 31.9 (37.4) | 30.5 (36.0) | 101.9 (109.4) | 36.0 (42.7) |
| Game's menu, 1920 × 1080 | 62.6 (68.1) | 59.7 (65.2) | 128.3 (135.7) | 71.1 (77.9) |
| Game's menu, 3840 × 2160 | 233.6 (239.1) | 221.0 (226.4) | 277.7 (285.0) | 265.9 (272.8) |

The editor takes 17 draws and 2,790,304 bytes, I 20 draws and 977,736 bytes; the game's menu 3
draws and 598,336 bytes, I 287 draws, one at each change of kind, and 207,416 bytes. The CPU
records the menu in 19 to 32 µs with S, S2 and T, in 258 to 277 µs with I, and writes the items'
bytes in 128 to 150 µs for the menu, 529 to 562 µs for the editor, alike in every layout. On
OpenGL ES, which gives no GPU time, the CPU's submission and the frame's time to the end of the
wait:

| Screen, size | I, submit | I, frame | T, submit | T, frame |
|---|---|---|---|---|
| Editor, 1280 × 800 | 157 | 331 | 228 | 472 |
| Editor, 3840 × 2160 | 164 | 571 | 236 | 731 |
| Game's menu, 1280 × 800 | 398 | 548 | 114 | 258 |
| Game's menu, 3840 × 2160 | 409 | 739 | 120 | 487 |

The effects on the game's menu, on Vulkan; S2 did not run them:

| Effect | Size | S | I | T | Bytes, S and T; I |
|---|---|---|---|---|---|
| Each panel turned, one card scaled | 1280 × 800 | 35.9 (41.4) | 109.5 (117.1) | 41.9 (48.5) | 614,672; 329,844 |
| Each panel turned, one card scaled | 1920 × 1080 | 69.9 (75.4) | 136.4 (144.0) | 82.3 (89.0) | 614,672; 329,844 |
| Each panel turned, one card scaled | 3840 × 2160 | 255.8 (261.3) | 289.7 (297.4) | 303.5 (310.1) | 614,672; 329,844 |
| A panel drawn apart and faded | 1280 × 800 | 38.4 (47.4) | 110.8 (128.6) | 44.0 (55.8) | 598,624; 321,440 |
| A panel drawn apart and faded | 1920 × 1080 | 72.6 (81.7) | 138.6 (156.5) | 84.4 (104.0) | 598,624; 321,440 |
| A panel drawn apart and faded | 3840 × 2160 | 260.8 (274.2) | 294.6 (312.5) | 306.8 (322.9) | 598,624; 321,440 |
| The same, the panel's texture kept | 1280 × 800 | 31.4 (36.2) | 86.1 (94.5) | 36.4 (42.5) | 375,216; 202,972 |
| The same, the panel's texture kept | 1920 × 1080 | 61.6 (66.4) | 111.2 (119.6) | 72.3 (78.4) | 375,216; 202,972 |
| The same, the panel's texture kept | 3840 × 2160 | 232.3 (237.1) | 256.4 (264.8) | 275.0 (281.0) | 375,216; 202,972 |
| Four glass panels, one break of the pass | 1280 × 800 | 93.6 (163.8) | 164.8 (237.4) | 99.0 (176.9) | 1,152,560; 614,068 |
| Four glass panels, one break of the pass | 1920 × 1080 | 148.5 (220.5) | 211.7 (282.6) | 158.9 (235.8) | 1,155,440; 615,588 |
| Four glass panels, one break of the pass | 3840 × 2160 | 454.1 (545.5) | 473.7 (557.7) | 489.6 (585.7) | 1,155,440; 615,588 |
| Twenty paths of flattened curves | 1280 × 800 | 107.6 (111.8) | 83.5 (88.4) | 118.0 (122.8) | 22,336; 20,732 |
| Twenty paths of flattened curves | 1920 × 1080 | 190.2 (194.2) | 140.4 (145.2) | 205.5 (210.3) | 22,336; 20,732 |
| Twenty paths of flattened curves | 3840 × 2160 | 712.6 (716.8) | 508.8 (513.7) | 766.5 (771.4) | 22,336; 20,732 |

A glass panel's blur takes 9 to 13 µs at 1920 × 1080 and 13 to 20 µs at 3840 × 2160; drawing the
canvas's texture onto the window some 9.5 and 30 µs. With 100,000 items, the CPU makes them in
3.1 ms and writes their 14.4 MB in 2.9 ms; one box moved and written alone, 144 bytes, takes
1.2 µs; the passes then take, at 1280 × 800 and 3840 × 2160, 69.3 and 193.3 µs with S, 153.2 and
223.9 with I, 70.7 and 206.1 with T. On OpenGL ES at 1920 × 1080, T submits the 100,000 items
written whole in 1,031 µs, the box alone in 83 µs.

Every layout's images are within 2/255 of S's on Vulkan, except the turned panels on radeonsi's
OpenGL ES: up to 10/255; T's 16 pixels above 3/255 all lie in the images of colours drawn turned,
and some 200 more within 2/255.

On lavapipe, Mesa 26.2.4, on the AMD Ryzen 7 5800X, from a load of 2.0, which lavapipe's own
threads raise; the passes, in ms:

| Screen, size | S | S2 | I | T |
|---|---|---|---|---|
| Editor, 1280 × 800 | 21.5 | 17.2 | 11.8 | 17.3 |
| Editor, 1920 × 1080 | 34.8 | 27.6 | 15.9 | 28.5 |
| Editor, 3840 × 2160 | 107.3 | 84.4 | 36.4 | 87.0 |
| Game's menu, 1280 × 800 | 9.8 | 8.1 | 7.7 | 9.6 |
| Game's menu, 1920 × 1080 | 19.6 | 16.1 | 10.0 | 16.4 |
| Game's menu, 3840 × 2160 | 75.6 | 61.0 | 24.8 | 64.9 |

The effects at 1920 × 1080, in the same order, S, I and T: the turned panels 29.8, 14.4 and 20.0;
the panel drawn apart 28.6, 13.7 and 21.8; the glass 47.9, 25.3 and 34.1; the paths 26.3, 19.2 and
22.4. A CPU that draws spends its time in each fragment's reads, of which I has the fewest: it ranks
I first, against the GPU. The runs' whole output is not kept; the prototype's README says how
to run them again.

CI, at `37ac9b1`: the milestone changed documents alone, which run the `Docs` workflow alone:

| Job | Time |
|---|---|
| Docs' `links` | 21 s |

## M6d.6, the series of headless compositors

The editor and *Hunna Wood* in five headless compositors
([ADR-0240](adr/0240-the-series-input-its-table-and-its-jobs.md)), on 2026-11-11, at the
revision of nixpkgs that `flake.lock` pins. What each compositor gives, and what differs, is in
[`compositors.md`](compositors.md).

The `compositors` workflow's jobs on CI's runners, `ubuntu-24.04`, at `e59a3c0`, the code of this
milestone before its review's changes, each from its start to its end; the series' own step, and
the series' shell fetched beside the default one:

| Job | Whole | The series | The series' shell |
|---|---|---|---|
| `compositors (weston)` | 14 min 21 s | 13 min 12 s | 15 s |
| `compositors (sway)` | 15 min 8 s | 14 min 8 s | 11 s |
| `compositors (kwin)` | 14 min 5 s | 12 min 55 s | 12 s |
| `compositors (mutter)` | 9 min 27 s | 8 min 25 s | 9 s |
| `compositors (x11)` | 12 min 24 s | 11 min 15 s | 12 s |

Each job stays within its limit of 20 minutes; most of each is the build of the editor, the game
and the tests, which each job makes alone.

What the series' shell, `nix develop .#compositors`, adds to the store beyond the default shell,
unpacked, by `nix path-info --recursive` on the two shells and on each package:

| Package | Beyond the default shell |
|---|---|
| The whole shell | 712 MB |
| KWin 6.7.5 | 360 MB |
| Mutter 50.4 | 203 MB |
| Xvfb 21.1.24 | 3 MB |
| libei 1.6.0, xdotool, wayland-utils 1.3.0 | 0 MB |
| Development outputs that the shell's inputs bring (Vulkan's headers, GTK's and others) | 146 MB |

The intervals between the starts of the `window` example's frames, at most 240 a second, 590 of
them after the first 10, by `scripts/frame-pacing.py`, drawn with lavapipe. KWin's output refreshes
at 60 Hz alone; Xvfb gives no rate. In CI, at `e59a3c0`:

| Compositor | Frames a second | Mean | Standard deviation | 99th percentile | Longest |
|---|---|---|---|---|---|
| Weston | 240.00 | 4.167 ms | 0.020 ms | 4.219 ms | 4.243 ms |
| sway | 224.98 | 4.445 ms | 0.055 ms | 4.588 ms | 5.175 ms |
| KWin, at 60 Hz | 59.94 | 16.683 ms | 0.500 ms | 17.568 ms | 17.658 ms |
| Mutter | 233.17 | 4.289 ms | 0.944 ms | 5.957 ms | 24.884 ms |
| X11 | 238.48 | 4.193 ms | 0.392 ms | 5.430 ms | 10.125 ms |

On a Ryzen 7 5800X, in a debug build, no cargo nor rustc running as each run started, though the
machine's load was 2 to 6, the other track of the stage and the milestone's review working beside:

| Compositor | Frames a second | Mean | Standard deviation | 99th percentile | Longest |
|---|---|---|---|---|---|
| Weston | 240.00 | 4.167 ms | 0.016 ms | 4.217 ms | 4.227 ms |
| sway | 226.48 | 4.415 ms | 0.075 ms | 4.670 ms | 4.802 ms |
| KWin, at 60 Hz | 59.97 | 16.675 ms | 0.877 ms | 19.444 ms | 22.688 ms |
| Mutter | 237.95 | 4.203 ms | 0.867 ms | 4.256 ms | 25.200 ms |
| X11 | 240.00 | 4.167 ms | 0.073 ms | 4.244 ms | 5.316 ms |

What it says:

- Weston paces the frames on its signal within 0.02 ms; sway's frames fall a little behind 240 a
  second, about 4.4 ms each, steadily.
- Mutter's frames keep their median at the refresh, but some come up to 25 ms late, in CI and on
  the machine alike: the widest spread of the series. X11, which gives no refresh rate, follows the
  engine's own schedule.
- These times are the frames' starts, which the engine reads today; when a frame shows is M6d.7's,
  through each platform's presentation times
  ([ADR-0227](adr/0227-the-time-that-motion-reads.md)).

## M6d.2b2b3, tokens

Tokens named by own styles and read when the interface takes its themes
([ADR-0238](adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md)), on 2026-11-11.
*Before* is `ba7c3e7`, the code of M6d.2b2b2; *after* is the milestone's working tree. The
counters, in a debug build, as CI counts them, on lavapipe:

| Benchmark | Counter | Before | After |
|---|---|---|---|
| `ui.tree.look` | elements styled by the frame of the change, own styles naming tokens | not counted | 12 487 |
| `ui.tree.look` | allocations of the frame of the change, own styles naming tokens | not counted | 109 804 |
| `ui.tree.look` | allocations of the frame of the change for each element styled, own styles naming tokens | not counted | 8.79, at most 20 |
| `ui.tree.look` | bytes allocated by the frame of the change | 24 487 625 | 24 488 489 |
| `ui.tree.look` | bytes allocated by the frame after the size of text changed | 91 044 670 | 91 045 534 |

What it says:

- A tree whose labels' and buttons' own styles name a role and a token of its own allocates, in
  the frame after its colours changed, 348 times more than the same tree whose labels and buttons
  have no own style, for 12 487 elements styled: 8.79 allocations for each element against 8.77,
  within ADR-0232's target of 20. The 348 count the own styles and the tokens together.
- Two frames allocate 864 bytes more, the same number of times, 0.0035 % of the frame after a
  change of colours and 0.0009 % of the frame after a change of the size of text: each own style
  kept in the styles made holds a list of its tokens, empty when it names none, 24 bytes. The owner
  accepted it. A declaration of an own style keeps the place of its token in the room that its
  state leaves, so that it takes no more room than before: holding the token itself, it took
  twice its 56 bytes, and the frame that builds the editor's inspector again allocated 1.5 %
  more bytes.
- Reading the look's documents allocates as before, 127 736 times: the tokens' values are read
  into the table of the read before, kept for its room.
- Every other counter of every benchmark is unchanged; so are the `interface` example's image,
  compared before and after on lavapipe, and the two fixtures of styles.

The times of the frames, and of compiling `uwui` with its setters generic, were not measured: the
machine was not at rest, the other track of the stage building and running its tests.

GitHub's hosted runners, on 2026-11-11, each run green, on `2ef0262`:

| Job | Time |
|---|---|
| `check (lint)` | 9 min 52 s |
| `check (headless)` | 16 min 48 s |
| `check (windowed)` | 8 min 52 s |
| `check (game)` | 12 min 50 s |
| `windows (tests)` | 13 min 59 s |
| `windows (render)` | 6 min 56 s |
| `windows (game)` | 7 min 9 s |
| `windows-release` | 16 min 28 s |
| Docs' `links` | 25 s |

Every job stays within its limit of 20 minutes. `check (headless)` took 35 s more than on `72ff330`,
its margin now 3 min 12 s; `windows-release` 2 min 38 s more, its margin 3 min 32 s. Both run the
whole workspace's build; the milestone adds two files of tests and no crate.

## M6d.2b2b2, the user's layer over what code writes

The user's size of text through the cascade and a list's rows, given as `Preferences` with less
motion ([ADR-0237](adr/0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md)),
on 2026-11-11. *Before* is `cd5a71e`, the code of M6d.2b2b1; *after* is the milestone's working
tree. The counters, in a debug build, as CI counts them, on lavapipe:

| Benchmark | Counter | Before | After |
|---|---|---|---|
| `ui.tree.look` | elements styled by the frame after the size of text changed, to 200 % | not counted | 12 487 |
| `ui.tree.look` | allocations of the frame after the size of text changed | not counted | 894 229 |
| `ui.tree.look` | bytes allocated by the frame after the size of text changed | not counted | 91 044 670 |
| `ui.tree.look` | allocations of the same tree's first frame at 200 % | not counted | 894 720 |
| `ui.tree.look` | allocations of the frame after the size of text changed for each element styled, at most the first frame's | not counted | 71.61, at most 71.66 |

What it says:

- A change of the size of text lays every text out again: it allocates as much as the tree's first
  frame at that size, its target, 71.6 times for each element. At 200 % the labels of the tree's
  panels, 300 logical pixels wide, break into more lines than at 100 %, whose first frame
  allocates 67.2 times for each element.
- Every other counter of every benchmark is unchanged; so are the images. The fixture of the
  `widgets` example's styles was recorded again: 126 of its lines changed, the labels and values
  whose width the example now writes as a least width, which draw the same.

The times of the frames were not measured, by the owner's choice: the machine could not be at
rest, other sessions building by turns. The first frame at 100 %, 67.2 allocations for each
element, was read from a run of the benchmark during the milestone, before its counter compared
the frame at 200 %; no counter keeps it. The times of compiling were not measured: the milestone
adds no crate and no dependency.

GitHub's hosted runners, on 2026-11-11, each run green, on `72ff330`:

| Job | Time |
|---|---|
| `check (lint)` | 8 min 12 s |
| `check (headless)` | 16 min 13 s |
| `check (windowed)` | 9 min 24 s |
| `check (game)` | 11 min 48 s |
| `windows (tests)` | 13 min 51 s |
| `windows (render)` | 6 min 49 s |
| `windows (game)` | 6 min 9 s |
| `windows-release` | 13 min 50 s |
| Docs' `links` | 20 s |

Every job stays within its limit of 20 minutes. `check (headless)` took 5 min 4 s more than on
`e25112c`, every part of it about 1.5 times as long, its build 6 min 10 s against 4 min 6 s, its
tests 6 min 14 s against 4 min 17 s, its cook and samples alike: a slower runner, not the
milestone's few tests; its margin is 3 min 47 s. `windows (tests)` ran both fixtures of styles on
Windows, and the test at 200 %.

## M6d.2b2b1, transitions and the frame of a change of theme

Transitions reading the derive's colours and sharing what they show, and drawing again without
copying a text's key and style
([ADR-0232](adr/0232-transitions-read-the-derives-colours-and-share-what-they-show.md)), on
2026-11-11. *Before* is `b32daaa`, the code of M6d.2b2a; *after* is the milestone's working tree.
The counters, in a debug build, as CI counts them, on lavapipe:

| Benchmark | Counter | Before | After |
|---|---|---|---|
| `ui.tree.look` | allocations of the frame of the change | 393 706 | 109 456 |
| `ui.tree.look` | bytes allocated by the frame of the change | 36 988 023 | 24 487 625 |
| `ui.tree.look` | allocations of the frame of the change for each element styled, at most 20 | 31.53 | 8.77 |
| `ui.tree.look` | allocations of a frame of the transition, its first | about 381 000 | 96 763 |
| `ui.tree.look` | bytes allocated by a frame of the transition, its first | not counted | 22 270 483 |
| `ui.tree.look` | allocations of a frame of the transition for each element drawn, at most 19 | 30.51 | 7.75 |
| `ui.tree.look` | elements styled by the frame of the change, drawn by a frame of the transition | 12 487 | 12 487 |
| `ui.tree.look` | frames of 240 Hz of the transition | 29 | 29 |
| `ui.tree.editor` | allocations of a frame after one value of the inspector changed | 32 163 | 32 088 |
| `ui.tree.editor` | bytes allocated by a frame after one value of the inspector changed | 7 157 318 | 7 147 614 |

What it says:

- The frame after a change of the look's colours allocates 72 % less: each element moving no
  longer clones its style, some eleven allocations, the elements that move alike sharing one
  style shown, about a dozen in this tree; and drawing a label again, whose text the cache holds,
  no longer copies the cache's key and the text's style.
- Each frame of the transition allocates 75 % less, for the same two reasons. The figure *before*
  was counted on 2026-11-11 by a copy of the benchmark that matched CI's counter of the frame of
  the change; no counter of a frame of the transition existed. Every later frame of the transition
  allocates within a few allocations of the first, the last a little less, its style no longer
  shown.
- What remains is drawing each element again, its lists of primitives made anew
  ([`debt.md`](debt.md)).
- `ui.tree.editor`'s frame after a value changed draws again some labels, which no longer copy
  their key; its other counters, and the other benchmarks', are unchanged; so are the fixtures of
  the styles and the images.
- The tree holds a dozen styles, all its elements moving alike in groups; a tree whose elements
  each go to a style of their own shares nothing, and the map of styles shown then keeps room for
  each element moving, about 3 MB for 12 487, for the interface's life.

The times of the frames were not measured: the owner's machine could not be at rest, other
sessions building throughout. The times of compiling were not measured either: the milestone adds
no crate and no dependency.

GitHub's hosted runners, on 2026-11-11, each run green, on `e25112c`:

| Job | Time |
|---|---|
| `check (lint)` | 9 min 22 s |
| `check (headless)` | 11 min 9 s |
| `check (windowed)` | 9 min 43 s |
| `check (game)` | 8 min 1 s |
| `windows (tests)` | 14 min 25 s |
| `windows (render)` | 7 min 37 s |
| `windows (game)` | 7 min 50 s |
| `windows-release` | 12 min 19 s |
| Docs' `links` | 20 s |
| uwui's `check`, on its `a28fe47` | 16 s |

Every job stays within its limit of 20 minutes; `windows (tests)`, the longest, has 5 min 35 s of
margin. `windows (tests)` ran both fixtures of styles on Windows, and the tests of transitions.

## M6d.9a1a, the development channel on uwui's core for agents

On 2026-11-12, on an AMD Ryzen 7 5800X, the machine at rest (load 0.2, no `cargo` nor `rustc`
running), no GPU: a request of the development channel answered at once, on the loopback, with 64
bytes after its answer, the median of 21, as `slime-channel`'s test
`a_request_is_answered_in_well_under_the_delay_of_an_acknowledgement` measures it, in a release
build (`cargo test --release -p slime-channel a_request_is_answered -- --nocapture`). Before is
`0dbe7e4`, the channel's binary encoding through the engine's reflection, built in a worktree of
its own; after is `b923757`, M6d.9a1a's, JSON-RPC a message a line through `uwui-agent`. Five passes
each, alternated; the median of the five.

| Measure | Before | After |
|---|---|---|
| a request of the development channel, answered at once, median of 21, release | 48.2 µs | 42.8 µs |
| its five passes | 43.1 to 52.5 µs | 41.4 to 44.8 µs |

M6c.0's 0.14 ms came from the same test, its build not recorded, before later changes of the machine
and the toolchain: it is not compared here. The answer did not grow with JSON; most of it is the
loopback and the threads that hand a request to the game and its answer back.

GitHub's hosted runners, on 2026-11-12, each run green, on `b923757`:

| Job | Time |
|---|---|
| `check (lint)` | 7 min 24 s |
| `check (headless)` | 14 min 32 s |
| `check (windowed)` | 9 min 8 s |
| `check (game)` | 15 min 27 s |
| `check (gles)` | 4 min 4 s |
| `windows (tests)` | 15 min 17 s |
| `windows (render)` | 7 min 4 s |
| `windows (game)` | 10 min 51 s |
| `windows-release` | 16 min 20 s |
| `compositors (weston)` | 13 min 29 s |
| `compositors (sway)` | 19 min 37 s |
| `compositors (kwin)` | 11 min 3 s |
| `compositors (mutter)` | 16 min 59 s |
| `compositors (x11)` | 13 min 16 s |
| Docs' `links` | 20 s |

Every job stays within its limit of 20 minutes; `compositors (sway)` has 23 s of margin. Against its
run on `5291eb1`, 11 min 35 s, each of its builds took about half again as long, a slower runner,
and the zone was cooked whole, 3 min 2 s against 3 s, where that run found it cooked, its cooker
changed with `slime-serialize`; the agent's session that drives the game, `drive`, took 2 min 43 s against 1 min 56 s.

## M6d.5a, the host's API tried on a prototype

On 2026-11-12, on an AMD Ryzen 7 5800X, in a release build, single runs, the CPU's load not
recorded: the throwaway prototype of [ADR-0249](adr/0249-the-hosts-api-its-items-and-their-table.md),
branch `m6d5a-prototype` at `9e48a0e`, its test `cost_over_a_field_of_4000_bytes`
(`cargo test -p uwui --release --test host_prototype -- --ignored --nocapture`). A single-line field
holds 4,000 bytes, 2,590 characters; the host asks its questions as an input method would.

| Question | Time |
|---|---|
| a character's rectangle | 109 to 122 µs, of which parley's selection 101 to 109 µs |
| the caret's rectangle | 15 µs, of which parley's caret 3.5 µs |
| the character at a point | 13.5 µs, of which parley's cluster 3.1 µs |
| the text around the caret, its UTF-16 offsets computed | 6.1 µs |
| the text laid out again after an edit, then a question | 1.1 ms, of which shaping 0.88 ms |

Parley's selection walks the whole line, wherever the character lies. Shaping is what each edit
pays already. Against the editor's frame of 4.17 ms at 240 Hz ([`ROADMAP.md`](ROADMAP.md#4-quality-bar)),
an edit with its question takes about a quarter; a host that asks a rectangle for each character
of a composition pays the first line's figure for each. The lowest machine is not measured, nor
was it at M6d.5c, none being set up; M6d.5c1 counts a pointer's move in uwui's benchmarks. These
are not benchmarks.

How long CI's jobs took on `226ae4c`, the commit that decided the host's API, on 2026-11-13: the
documents changed and `uwui`'s colours' paths compiled only with the tests that read them.

| Job | Time |
|---|---|
| `check (lint)` | 10 min 10 s |
| `check (headless)` | 17 min 12 s |
| `check (windowed)` | 9 min 13 s |
| `check (game)` | 9 min 34 s |
| `check (gles)` | 3 min 28 s |
| `windows (tests)` | 12 min 41 s |
| `windows (render)` | 7 min 0 s |
| `windows (game)` | 5 min 58 s |
| `windows-release` | 12 min 37 s |
| Docs' `links` | 23 s |

Every job stays within its limit of 20 minutes; the compositors' jobs did not run, no path of
theirs changed. Against M6d.9a1a's run above, `check (headless)` took 2 min 40 s more and
`check (lint)` 2 min 46 s more, while `check (game)`, `windows (tests)`, `windows (game)` and
`windows-release` took 3 to 6 min less: the runners vary, and uwui's one `cfg` explains neither.
`check (headless)` keeps 2 min 48 s of margin.

## M6d.5b, the times

The rest, on 2026-11-13, by the checks that the quality bar names, in `scripts/check.sh` on an AMD
Ryzen 7 5800X with lavapipe in a headless Weston, the machine not at rest (the other track
building): the editor at rest with a panel floating, then Hunna Wood paused and minimized, none of
their threads waking in 10 s, as before the milestone. No series ran on the RX 6900 XT, by the
owner's choice: the milestone changes the times given, no frame nor wake at rest.

The counters: the frame of a change of the editor's look (`ui.tree.look`) allocates as many times
as before and 24,688,729 bytes against 24,488,489, 0.8 % more: each transition's start names the
clock that it follows.

How long CI's jobs took on `ab9496e`, the commit that gave uwui the host's times, on 2026-11-13:

| Job | Time |
|---|---|
| `check (lint)` | 8 min 13 s |
| `check (headless)` | 12 min 14 s |
| `check (windowed)` | 10 min 16 s |
| `check (game)` | 11 min 51 s |
| `check (gles)` | 4 min 17 s |
| `windows (tests)` | 11 min 4 s |
| `windows (render)` | 7 min 0 s |
| `windows (game)` | 7 min 50 s |
| `windows-release` | 16 min 46 s |
| `compositors (weston)` | 11 min 39 s |
| `compositors (sway)` | 11 min 47 s |
| `compositors (kwin)` | 14 min 28 s |
| `compositors (mutter)` | 15 min 0 s |
| `compositors (x11)` | 14 min 44 s |
| Docs' `links` | 20 s |

Every job stays within its limit of 20 minutes. `windows-release` is the longest, with 3 min 14 s
of margin, 4 min 9 s more than at M6d.5a's run above; `check (headless)` took 4 min 58 s less. The
runners vary; the milestone adds no job and one test file of uwui.

## M6d.9a1b1, requests in flight

On 2026-11-13, on an AMD Ryzen 7 5800X: the most bytes held at once while `uwui-agent`'s reader
reads a request's line of 1 MiB, the reader's copy of the line counted, not the caller's, as
`uwui-agent`'s test
`a_request_s_line_costs_a_bounded_multiple_of_its_size_as_it_is_read` counts them through a global
allocator of its own (`cargo test --release -p uwui-agent --test request_memory -- --nocapture`).
Its parameters are one shape repeated in a list; a debug build gives the same figures. It replaces
the 17 times that M6d.9a1a's documents gave, which no measure held.

| A line of 1 MiB whose parameters are a list of | Bytes held at most | Times the line |
|---|---|---|
| numbers, `0` | 26,214,415 | 25.0 |
| empty lists, `[]` | 26,214,415 | 25.0 |
| empty texts, `""` | 26,214,415 | 25.0 |
| empty objects, `{}` | 26,214,415 | 25.0 |
| short texts, `"lamp 3"` | 7,733,269 | 7.4 |
| the world's commands, as an editor sends them | 2,471,605 | 2.4 |

The worst holds a value of 32 bytes for each two or three of the line, in a list that doubles its
room as it grows, its last two blocks held at once. The test fails past 25.5 times. It measures the
reader alone: the game then reads the value into its request, both held a moment, which no test
measures.

On 2026-11-13, on the same machine at rest before each series (load under 1, no `cargo` nor
`rustc` running), in release builds:

- A request of the development channel answered at once, on the loopback, with 64 bytes after its
  answer, the median of 21, as `slime-channel`'s test
  `a_request_is_answered_in_well_under_the_delay_of_an_acknowledgement` measures it
  (`cargo test --release -p slime-channel a_request_is_answered -- --nocapture`). Before is
  `e4616dc`, one request in flight a connection, built in a worktree of its own; after is this
  milestone's, each connection with a reader and a writer. Five passes each, alternated; the median
  of the five.

| Measure | Before | After |
|---|---|---|
| a request of the development channel, answered at once, median of 21 | 41.1 µs | 45.3 µs |
| its five passes | 38.3 to 42.1 µs | 43.7 to 46.3 µs |

  The writer's thread adds a hop between the game's answer and the socket.

- A headless run's frames by the wall's clock, over six seconds after two of warming, while seven
  connections each keep sixteen lists of a thousand entities in flight, of a world of five thousand,
  and an eighth asks how the frames went, as `slime-app`'s test `frames_with_agents_at_the_bounds`
  measures it (`cargo test --release -p slime-app --features development --test channel --
  --ignored frames_with_agents --nocapture`). Each frame rests 1 ms, as the channel's tests do.
  With the budget of a frame's start, 0.5 ms, and without it, the same build with the budget made
  endless. Three passes each, alternated; the load rose to 2.4 during them, the measure's own nine
  threads.

| A frame, with agents at the bounds | With the budget | Without it |
|---|---|---|
| frames in six seconds | 1,424 to 1,474 | 0 |
| median | 4.03 to 4.10 ms | none |
| longest | 5.84 to 12.22 ms | none |

  Without the budget, the agents' requests come back as fast as the frame's start takes them, and
  it never ends. With it, one round, a list of each connection, passes the 0.5 ms alone: the frame
  holds the round and the rest.

GitHub's hosted runners, on 2026-11-13, each run green, on `8581945`:

| Job | Time |
|---|---|
| `check (lint)` | 8 min 27 s |
| `check (headless)` | 17 min 52 s |
| `check (windowed)` | 11 min 31 s |
| `check (game)` | 9 min 59 s |
| `check (gles)` | 4 min 14 s |
| `windows (tests)` | 15 min 29 s |
| `windows (render)` | 7 min 6 s |
| `windows (game)` | 7 min 10 s |
| `windows-release` | 15 min 56 s |
| `compositors (weston)` | 13 min 20 s |
| `compositors (sway)` | 16 min 2 s |
| `compositors (kwin)` | 13 min 28 s |
| `compositors (mutter)` | 14 min 2 s |
| `compositors (x11)` | 14 min 42 s |
| Docs' `links` | 21 s |

Every job stays within its limit of 20 minutes; `check (headless)` keeps 2 min 8 s of margin, the
least, and `compositors (sway)` took 3 min 35 s less than at M6d.9a1a, on another runner.

## M6d.5c1, input as the host's events

A pointer's move, on 2026-11-13, by `editor.hierarchy.100k` in a release build on an AMD Ryzen 7
5800X, the machine not at rest (the other track building, a load of 1.4 to 6.0): the move from a
row of the hierarchy over 100,000 entities to the next and back, each move hovering another row,
given before any frame, as a host gives it; the code before and after a move walked the tree once
for the hover and the rest, built and run in turn, twice.

| Series | A move, median of 15 | Allocations |
|---|---|---|
| Before, first | 3.2 µs | 0 |
| After, first | 1.6 µs | 0 |
| Before, second | 3.2 µs | 0 |
| After, second | 1.6 µs | 0 |

What it says: a move costs half as much, and allocates nothing, which `benches/counters.txt` now
checks. A mouse at 1,000 Hz gives about four moves for each frame at 240 Hz; the engine gives them
uwui as one, so the hierarchy costs about 1.6 µs of hit test for each frame of motion; the editor's
whole window, whose hit test walks every root's elements, costs more, not measured. The lowest
machine of the quality bar was not measured: it is not set up yet. Input to present waits for
M6d.5c3 and the owner's go on the GPU.

The window's input through the engine, on the same day, by `scripts/input-to-present.sh` on
lavapipe in a headless sway at 240 Hz, the machine not at rest: an end-to-end check that the
engine's events and its moves given as one reach the interface, not a measure of the budget,
which waits for the GPU. The editor gave 141 images after 200 inputs apart, clicks, the wheel, keys
and moves, a median of 10.4 ms and a 99th percentile of 13.9 ms; and 441 after 1,000 moves in a
row, 11.6 ms and 17.2 ms. *Hunna Wood* drew a frame every few seconds on lavapipe, too few to give
any; both ended well.

How long CI's jobs took on `b3cc0cd`, the commit that gave uwui its input as the host's events, on
2026-11-13:

| Job | Time |
|---|---|
| `check (lint)` | 7 min 49 s |
| `check (headless)` | 17 min 1 s |
| `check (windowed)` | 9 min 34 s |
| `check (game)` | 13 min 27 s |
| `check (gles)` | 3 min 17 s |
| `windows (tests)` | 15 min 14 s |
| `windows (render)` | 5 min 54 s |
| `windows (game)` | 5 min 56 s |
| `windows-release` | 16 min 5 s |
| `compositors (weston)` | 11 min 29 s |
| `compositors (sway)` | 15 min 48 s |
| `compositors (kwin)` | 10 min 27 s |
| `compositors (mutter)` | 13 min 12 s |
| `compositors (x11)` | 12 min 14 s |
| Docs' `links` | 19 s |

Every job stays within its limit of 20 minutes. `check (headless)` is the longest, with 2 min 59 s
of margin; `windows (tests)` took 15 min 14 s, on another runner. The part adds one test file of
uwui and two tests of `slime-app`.

## M6d.9a1b2, events

On 2026-11-13, on an AMD Ryzen 7 5800X, in release builds, each series started at rest: a load
under 1 over a minute, no `cargo` nor `rustc` running; the loads over five and fifteen minutes still
near 4, from builds that had just ended. Before is `c08c90c`, built in a worktree of its own; after
is this milestone's.

- A request of the development channel answered at once, as M6d.9a1b1 measured it
  (`cargo test --release -p slime-channel --lib a_request_is_answered -- --nocapture`), five passes
  each, alternated; the median of the five. The writer now looks at the subscriptions' cells
  between two items: no change.

| Measure | Before | After |
|---|---|---|
| a request of the development channel, answered at once, median of 21 | 46.3 µs | 45.7 µs |
| its five passes | 45.8 to 50.6 µs | 39.6 to 49.8 µs |

- An event, `play/time`, from the game's publication to the tool's function that the client calls
  on its own thread, on the loopback, 21 times: alone, then each behind the answer to a request of
  a 4K capture's bytes, 3840 × 2160 × 4, which the game leaves for the writer just before it
  publishes, as `slime-channel`'s test `an_event_behind_a_capture` measures it
  (`cargo test --release -p slime-channel --lib an_event_behind -- --ignored --nocapture`). Three
  passes; the median of their medians, and the range of their slowest.

| An event to the tool | Median of 21 | Slowest of 21 |
|---|---|---|
| alone | 22.4 µs | 28.9 to 62.2 µs |
| behind a 4K capture | 13.2 ms | 24.0 to 27.7 ms |

  Behind a capture, the event waits for the writer to write its 33 MB, then goes out before the
  next answer ([ADR-0254](adr/0254-events-declared-in-the-welcome-replaced-in-the-carrier-published-at-the-frames-end.md)).
  A tool that wants its events at once asks for no capture on their connection, as the editor's
  Play does.

- Play's time to control on a level of 100 000 entities, `play.level.100k`
  (`cargo run --release -p slime-benches -- play.level.100k`), before measured by asking the game
  every 5 ms and after by the first `play/time` whose `held` is false; two passes each, alternated.

| `play.level.100k`, of 5 | Before | After |
|---|---|---|
| from the request to control, median, pass 1 | 1,358 ms | 1,503 ms |
| from the request to control, 90th percentile, pass 1 | 1,361 ms | 2,495 ms |
| from the request to control, median, pass 2 | 1,325 ms | 1,330 ms |
| from the request to control, 90th percentile, pass 2 | 1,346 ms | 1,498 ms |
| of it, the editor writing the level, median, pass 1 | 374 ms | 448 ms |
| of it, the editor writing the level, 90th percentile, pass 1 | 379 ms | 804 ms |
| of it, the editor writing the level, median, pass 2 | 366 ms | 368 ms |
| of it, the editor writing the level, 90th percentile, pass 2 | 377 ms | 470 ms |

  The first pass after was disturbed: the editor writing the level, which the milestone does not
  change, took a fifth longer at the median and twice as long at the 90th percentile. In the
  second, the medians agree, and the 90th percentile is a tenth higher, as is the editor's writing,
  which the event does not touch: the event gives about the same time to control as the question
  it replaces, which two passes cannot tell apart better. ADR-0110's budget, at the 90th
  percentile, holds in each. *Hunna Wood*'s `play.zone` runs in a window on the owner's desktop, and was not
  measured again: a [check by hand](PLAN.md#checks-by-hand-waiting-for-the-owner).

- At rest, a game subscribed to by two connections wakes none of its channel's threads over a
  second, as `slime-app`'s test `a_run_on_demand_tells_its_subscribers_how_game_time_stands` counts
  their context switches on Linux; it runs on every check.

How long CI's jobs took on `15a6c2b`, the commit that sent tools the events they subscribed to, on
2026-11-13:

| Job | Time |
|---|---|
| `check (lint)` | 7 min 39 s |
| `check (headless)` | 13 min 55 s |
| `check (windowed)` | 7 min 33 s |
| `check (game)` | 9 min 26 s |
| `check (gles)` | 4 min 5 s |
| `windows (tests)` | 15 min 52 s |
| `windows (render)` | 7 min 10 s |
| `windows (game)` | 6 min 28 s |
| `windows-release` | 16 min 12 s |
| `compositors (weston)` | 11 min 32 s |
| `compositors (sway)` | 12 min 45 s |
| `compositors (kwin)` | 11 min 31 s |
| `compositors (mutter)` | 10 min 33 s |
| `compositors (x11)` | 11 min 7 s |
| Docs' `links` | 18 s |

Every job stays within its limit of 20 minutes. `windows-release` is the longest, with 3 min 48 s
of margin. The milestone adds a dozen tests of `slime-channel`, six of `uwui-agent`, one of
`slime-app` that runs a game, and a step to `slime-mcp`'s session of the editor.

## M6d.5c2, the clipboard and the host's questions

The host's questions over a field of one line of 4,000 bytes, on 2026-11-13, in a release build on
an AMD Ryzen 7 5800X, the machine not at rest: another project's tests ran, a load of 2.6 to 4.3.
The prototype of M6d.5a (`9e48a0e`, its test `cost_over_a_field_of_4000_bytes`) and this
milestone's test of the same name (`cargo test -p uwui --release --test questions -- --ignored
--nocapture`) were run in turn, three times each, over the prototype's text: ASCII, two-byte,
three-byte and four-byte characters, `word é 日 😀 ` repeated. Each figure is the range of the
three runs' means, each mean of 2,000 to 20,000 questions; the edit, of 200.

| Question | Prototype | M6d.5c2 |
|---|---|---|
| The text around the caret, 100 units of UTF-16 each side | 6.4 to 6.8 µs | 2.3 to 2.4 µs |
| The box of the last character | 123 to 125 µs | 19 to 20 µs |
| The box of the first character | 113 to 116 µs | 7.5 to 8.4 µs |
| The caret's box at the end | 15.5 to 16.0 µs | 14.7 to 16.5 µs |
| The character at a point | 12.6 to 13.0 µs | 12.6 to 13.0 µs |
| An edit to a new text, Backspace, then a box | 1.14 to 1.18 ms | 1.03 to 1.09 ms |

What it says:

- A character's box costs a sixth of the prototype's: a range of one direction on one line is
  boxed from its ends' carets, where the prototype walked the whole line.
- The text around the caret costs a third: its places are counted once from the text's start,
  then within the excerpt.
- The caret's box and the character at a point cost what they did.
- An edit with its question still takes about a quarter of the editor's frame at 240 Hz: the
  shaping of the edit's new text, which every edit pays, holds it.
- A host that asks a box for each character of a composition, as Android and the web do, pays
  about 20 µs for each.

Over two other texts of 4,000 bytes, the same day, twice each, this milestone's code alone:

| Question | Latin and Hebrew, `word שלום ` repeated | A few of those, then one word of 3,400 letters |
|---|---|---|
| The text around the caret | 2.4 µs | 2.3 µs |
| The box of the last character | 30 to 31 µs | 278 to 286 µs |
| The caret's box at the end | 22 µs | 196 to 199 µs |
| The character at a point | 18 to 19 µs | 204 to 208 µs |
| An edit, Backspace, then a box | 1.34 to 1.38 ms | 0.92 to 0.95 ms |

Two directions cost about half as much again as one. A long word after text of two directions
costs ten times as much, the character at a point included, which calls parley alone: the cause is
not found ([`debt.md`](debt.md)). The lowest machine of the quality bar is not measured: none is
set up yet. These are not benchmarks.

How long CI's jobs took on `5bb456e`, the commit that gave uwui's clipboard to its host, on
2026-11-13:

| Job | Time |
|---|---|
| `check (lint)` | 7 min 39 s |
| `check (headless)` | 11 min 56 s |
| `check (windowed)` | 11 min 31 s |
| `check (game)` | 9 min 35 s |
| `check (gles)` | 3 min 10 s |
| `windows (tests)` | 15 min 41 s |
| `windows (render)` | 5 min 50 s |
| `windows (game)` | 7 min 51 s |
| `windows-release` | 15 min 54 s |
| `compositors (weston)` | 10 min 23 s |
| `compositors (sway)` | 11 min 22 s |
| `compositors (kwin)` | 14 min 34 s |
| `compositors (mutter)` | 11 min 42 s |
| `compositors (x11)` | 11 min 0 s |
| Docs' `links` | 16 s |

Every job stays within its limit of 20 minutes. `windows-release` is the longest, with 4 min 6 s of
margin, and `windows (tests)` keeps 4 min 19 s; `check (headless)` took 5 min 5 s less than at
M6d.5c1, on another runner. The compositors' series found the clipboard as its table says on each.

## M6d.5c3a, input and outputs per target

A pointer's move, on 2026-11-14, by `editor.hierarchy.100k` in a release build on an AMD Ryzen 7
5800X, the machine not at rest (another project's tests building, a load near 15): the move from a
row of the hierarchy over 100,000 entities to the next and back, given before any frame, as a host
gives it; the code before, `bdd1ddd`, built apart, and after, each target's input kept apart, run
in turn, twice.

| Series | A move, median of 15 | Allocations | A frame standing still, median of 15 |
|---|---|---|---|
| After, first | 1.7 µs | 0 | 8.0 µs |
| Before, first | 1.6 µs | 0 | 8.2 µs |
| After, second | 1.6 µs | 0 | 8.3 µs |
| Before, second | 1.7 µs | 0 | 8.0 µs |

What it says: keeping each target's input apart, and finding a root's target at each walk of the
roots, costs nothing that this series can tell, under its noise of 0.1 µs; a move still allocates
nothing, and a frame standing still visits the same 638 elements. The lowest machine of the quality
bar is not measured: none is set up yet. Input to present waits for M6d.5c3b and the owner's go on
the GPU.

The window's input through the engine, on the same day, by `scripts/input-to-present.sh` on
lavapipe in a headless sway at 240 Hz, the machine not at rest: an end-to-end check that the
engine's input still reaches the interface on `TargetId::MAIN`, not a measure of the budget, which
waits for the GPU. The editor gave 147 images after 200 inputs apart, clicks, the wheel, keys and
moves, a median of 11.9 ms and a 99th percentile of 18.3 ms; and 436 after 1,000 moves in a row,
12.4 ms and 17.0 ms, near M6d.5c1's. *Hunna Wood* drew a frame every few seconds on lavapipe, too
few to give any; both ended well.

How long CI's jobs took on `fdd5d74`, the commit that gave each of uwui's roots its target, on
2026-11-14:

| Job | Time |
|---|---|
| `check (lint)` | 7 min 42 s |
| `check (headless)` | 13 min 54 s |
| `check (windowed)` | 11 min 25 s |
| `check (game)` | 12 min 55 s |
| `check (gles)` | 3 min 53 s |
| `windows (tests)` | 15 min 26 s |
| `windows (render)` | 6 min 54 s |
| `windows (game)` | 6 min 40 s |
| `windows-release` | 15 min 55 s |
| `compositors (weston)` | 13 min 14 s |
| `compositors (sway)` | 12 min 38 s |
| `compositors (kwin)` | 10 min 37 s |
| `compositors (mutter)` | 13 min 19 s |
| `compositors (x11)` | 10 min 31 s |
| Docs' `links` | 15 s |

Every job stays within its limit of 20 minutes. `windows-release` is the longest, with 4 min 5 s of
margin, and `windows (tests)` keeps 4 min 34 s. The part adds one test file of uwui, of 19 tests.

## M6d.9a1b3a, the interface's family in uwui

The check that the game built for shipping holds neither the development channel nor uwui's answers
to agents, shown to fail, on 2026-11-14: the step of `scripts/check.sh` run alone, on *Hunna Wood*
built with its default features first, each mutation undone and its files checked by their sha256.

| Mutation | The step |
|---|---|
| None | passes |
| `slime-app`'s manifest gives `uwui` its feature `agents` in every build | fails: the crate `uwui-agent` and uwui's feature `agents` |
| uwui's features read in the tree that is not inverted, as before | fails: the default build shows no `agents` there |
| `cargo tree` given a kind of edge that does not exist | fails: cargo's error |
| The game's binary missing, under a target directory of its own | fails: the binary is missing |
| A text that the binary does not hold, `interface/elementz` | fails: the default build lacks it |

What it says: the step reads `agents` where a feature that another turns on shows, which the tree
that it read before never showed; a text of `uwui::agents` that no call reaches is dropped by the
linker, and the trees find its crate and its feature then.

How long CI's jobs took on `52e5fe1`, the commit that moved the interface's family into uwui, on
2026-11-14; `check (headless)`, the `windows` jobs of tests and release, and `compositors
(mutter)` from their second run, a push of track B's having cancelled the first:

| Job | Time |
|---|---|
| `check (lint)` | 7 min 31 s |
| `check (headless)` | 12 min 48 s |
| `check (windowed)` | 11 min 22 s |
| `check (game)` | 10 min 28 s |
| `check (gles)` | 4 min 11 s |
| `windows (tests)` | 16 min 17 s |
| `windows (render)` | 6 min 47 s |
| `windows (game)` | 8 min 4 s |
| `windows-release` | 15 min 57 s |
| `compositors (weston)` | 9 min 41 s |
| `compositors (sway)` | 11 min 12 s |
| `compositors (kwin)` | 10 min 24 s |
| `compositors (mutter)` | 13 min 3 s |
| `compositors (x11)` | 12 min 3 s |
| Docs' `links` | 18 s |

Every job stays within its limit of 20 minutes. `windows (tests)` is the longest, with 3 min 43 s of
margin, and `windows-release` keeps 4 min 3 s. The part adds 8 tests, 3 of them uwui's, one of
which takes the place of a test of `slime-app`'s.


## M6d.5c3b, frames and times per target

The frame of a change of look, on 2026-11-14, by `ui.tree.look` in a release build on an AMD Ryzen
7 5800X, the machine not at rest (another project building, a load falling from 8 to 5): the code
before, `52e5fe1`, built apart, and after, each target its own frame, run in turn, three times
each.

| Series | A frame after the look's colours changed, median of 15 | Its transition's 29 frames, together, median of 15 |
|---|---|---|
| Before, first | 56.79 ms | 776.43 ms |
| After, first | 59.58 ms | 871.29 ms |
| Before, second | 51.00 ms | 672.04 ms |
| After, second | 51.94 ms | 671.43 ms |
| Before, third | 50.86 ms | 646.44 ms |
| After, third | 52.22 ms | 695.43 ms |

Both builds style the same 12,487 elements and allocate the same 109,456 times in the frame of the
change; the counters of `benches/counters.txt` are unchanged, a frame standing still allocating as
before. What it says: noting each change on its target and filtering by the element's target may
cost about 1 ms of the change's 51 ms, under the series' noise of some 6 ms between its first
rounds. One target alone is measured: the filters on two targets are read to cost one comparison
for each element waiting, not timed.

Input to present on the GPU, the owner's go, the same day, by `scripts/input-to-present.sh --gpu` on
the RX 6900 XT, its clock on `high`, in a headless sway at 240 Hz; before and after in turn, twice
each, the machine's load falling from 4.5 to 1.9, not at rest. The intervals from an input to the
end of the present of the image after it, before then after, each pair the two series:

| Program | Inputs | Median before | Median after | 99th percentile before | 99th percentile after |
|---|---|---|---|---|---|
| The editor, on the yard | apart | 0.71 ms, 0.97 ms | 0.71 ms, 0.70 ms | 13.05 ms, 13.17 ms | 12.87 ms, 1.55 ms |
| The editor, on the yard | in a row | 12.68 ms, 13.05 ms | 12.37 ms, 12.53 ms | 16.53 ms, 16.67 ms | 16.57 ms, 17.07 ms |
| *Hunna Wood* | apart | 8.81 ms, 9.35 ms | 8.80 ms, 8.75 ms | 16.48 ms, 16.66 ms | 16.58 ms, 16.42 ms |
| *Hunna Wood* | in a row | 12.64 ms, 12.94 ms | 12.81 ms, 12.53 ms | 16.86 ms, 17.40 ms | 16.74 ms, 16.56 ms |

What it says: the engine's input reaches its image as before; the editor and the game draw one
target, and take the path of one. The editor's 99th percentile still passes its frame of 4.17 ms,
an input waiting for the next frame of a schedule of 60 a second, as at M6c.Q2d
([`debt.md`](debt.md)); *Hunna Wood* stays within its two frames.

How long CI's jobs took on `d1c795d`, the commits that gave each target its frame, on 2026-11-14,
run again once track A's run of `52e5fe1` ended (each push to `main` cancels the run before it):

| Job | Time |
|---|---|
| `check (lint)` | 8 min 22 s |
| `check (headless)` | 17 min 15 s |
| `check (windowed)` | 10 min 51 s |
| `check (game)` | 12 min 5 s |
| `check (gles)` | 3 min 11 s |
| `windows (tests)` | 16 min 0 s |
| `windows (render)` | 7 min 12 s |
| `windows (game)` | 6 min 49 s |
| `windows-release` | 11 min 58 s |
| `compositors (weston)` | 14 min 5 s |
| `compositors (sway)` | 15 min 35 s |
| `compositors (kwin)` | 14 min 23 s |
| `compositors (mutter)` | 10 min 20 s |
| `compositors (x11)` | 12 min 40 s |
| Docs' `links` | 23 s |

Every job stays within its limit of 20 minutes. `check (headless)` is the longest, with 2 min 45 s
of margin, 3 min 21 s longer than at M6d.5c3a: the commit carries M6d.9a1b3a's tests too, and the
runners' times vary. `windows (tests)` keeps 4 min. The part adds 13 tests of uwui and one of
`slime-app`'s, in files that existed.

## M6d.5d1, each reader its changes, each target its tree

A change of look and the changes taken after it, on 2026-11-14, by a scratch test that the
repository does not keep: one root of 6,000 buttons and 6,000 texts, 12,001 elements, in the
editor's look, its colours changed from dark to light, then its main target's frame timed, then
one reader's changes taken; a release build on an AMD Ryzen 7 5800X, the machine not at rest
(track A's tests running, a load between 4 and 8). The code before, `05852fa`, built apart, and
after, run in turn three times each, then three times again after the closing review's fixes;
each figure the median of 12.

| Series | The frame of the change, no reader | The frame, one reader | The changes taken |
|---|---|---|---|
| Before, six rounds | 111.2 to 117.8 ms | 113.4 to 119.6 ms | 4.0 to 6.7 ms |
| After, six rounds | 112.4 to 121.7 ms | 114.5 to 121.4 ms | 4.2 to 7.8 ms |

What it says: a reader's own marks cost the change of look and its changes about what one list for
everyone cost, under the noise of a machine that was not at rest. With two readers, the frame and a
take stay in the same ranges. A frame standing still marks nothing, and the counters of
`benches/counters.txt` are unchanged.

How long CI's jobs took on `18b64cd`, the commit that gave each reader its changes, on 2026-11-14:

| Job | Time |
|---|---|
| `check (lint)` | 9 min 43 s |
| `check (headless)` | 14 min 45 s |
| `check (windowed)` | 8 min 32 s |
| `check (game)` | 9 min 18 s |
| `check (gles)` | 4 min 7 s |
| `windows (tests)` | 15 min 37 s |
| `windows (render)` | 6 min 3 s |
| `windows (game)` | 7 min 38 s |
| `windows-release` | 12 min 2 s |
| `compositors (weston)` | 12 min 29 s |
| `compositors (sway)` | 11 min 38 s |
| `compositors (kwin)` | 10 min 7 s |
| `compositors (mutter)` | 10 min 37 s |
| `compositors (x11)` | 15 min 9 s |
| Docs' `links` | 21 s |

Every job stays within its limit of 20 minutes. `windows (tests)` is the longest, with 4 min 23 s of
margin; `check (headless)` keeps 5 min 15 s. The part adds five tests of uwui and three of
`slime-app`'s, in files that existed.
