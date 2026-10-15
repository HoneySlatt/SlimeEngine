# Examples

Runnable programs that each demonstrate one engine feature. They belong to the `slime-examples`
package, which has no other purpose and is never published. Run them from the repository root,
inside `nix develop`:

```sh
cargo run --example <name> [-- <arguments>]
```

| Example | Shows | Arguments |
|---|---|---|
| [`hello`](hello.rs) | Diagnostics: logging through `tracing`, the `SLIME_LOG` filter, panic reporting | `--panic` panics on purpose |
| [`headless`](headless.rs) | An application without a window: frames back to back with simulated time, exit at a frame limit, profiling spans (`SLIME_LOG=trace`) | `--frames <count>` (default 600) |
| [`window`](window.rs) | A window in which the GPU draws the samples' triangle, which follows resizes; its events logged (window, keys, mouse; cursor motion with `SLIME_LOG=debug`); Escape, closing it or Ctrl+C quits cleanly. The sample of Stage 2 | `--frames <count>` closes it after that many frames and, with a window, fails if none of them drew the triangle; `--headless` runs the same program without a window; `--present-mode mailbox|fifo` chooses how images are shown (default `mailbox`) |
| [`offscreen`](offscreen.rs) | Rendering without a window: the GPU draws a triangle into an image, which is read back and written to a PNG file; works on software drivers (lavapipe, WARP) | `--output <file>` (default `triangle.png`) |
| [`scene`](scene.rs) | A scene cooked by the asset pipeline ([ADR-0069](../docs/adr/0069-the-asset-pipeline.md)) and drawn by the renderer (`slime-render`): the asset of [`assets/scene.glb`](assets/README.md), cooked first (below), or any scene that `slime-cook` cooked, with its physically based materials, lit in physical units by the lights its nodes place (the asset's sun, the light of its glowing ring and a spot light) or, for a scene without lights, by the sample's sun, and by a clear blue sky around the first directional light, drawn behind the scene with the sun's disc (above what the camera can see in the asset, whose sun is high) and a bloom that makes bright light glow, seen from a camera that orbits it until the mouse takes over (drag with the left button to turn, from level with the scene to nearly straight down, roll the wheel to come nearer or farther), with a dev overlay showing the frame time, the CPU time of the render, the GPU time of each pass (shadow, scene, sky, bloom, tone mapping), the draw calls and triangles, the draw calls for shadows, the CPU time taken by the sky once the scene was loaded, an exposure slider in a photographer's exposure values, EV100, and a bloom strength slider, from none to five times the sample's, and, for the sample's asset, a slider of how strongly its ring glows, from none to four times, which changes the ring's material while the example runs, found by its name (`LoadedScene::material`, `Material::set_values`) (F1 hides it); Escape, closing the window or Ctrl+C quits cleanly. It loads the scene in the background (`Renderer::load_scene_in_background`), its frames showing the sky's horizon until the scene is loaded. The sample of Stages 3a, 3b and 4a | `--assets <directory>` reads the cooked data of another directory (by default, `assets` beside the program when it holds a manifest, as in the Windows artifact below, and `target/cooked/examples` otherwise); `--scene <id>` draws another of its scenes, by its ID (a glTF file's default scene has the file's ID); `--frames <count>` closes it after that many frames that drew the scene, and fails if none did; `--watch` loads the scene again, keeping what did not change, when its cooked data changes, beside `slime-cook watch` running on its sources ([ADR-0076](../docs/adr/0076-hot-reload-of-cooked-data.md)) |

On Windows, the same commands work with the toolchain described in
[`CONTRIBUTING.md`](../CONTRIBUTING.md#on-windows). Each CI run on GitHub also publishes
`hello.exe`, `headless.exe`, `window.exe`, `offscreen.exe` and `scene.exe` built for Windows, as the
artifact `slime-examples-windows`, with the game, `hunnawood.exe`
([`games/hunnawood`](../games/hunnawood/README.md)) and its cooked zone, to run on a Windows machine
without a Rust toolchain. They link the C runtime of the MSVC toolchain statically, so they start on a
fresh Windows install too, without the Visual C++ Redistributable. Run them from a terminal (Windows
Terminal, PowerShell or cmd): started by a double click, their console closes with them, and an
error at startup disappears with it.

The `scene` example reads cooked data, which the cooker (`slime-cook`,
[`crates/slime-cook`](../crates/slime-cook/README.md)) makes from the sources in
[`assets/`](assets/README.md). Cook them once, and again after a source changes:

```sh
cargo run -p slime-cook -- cook examples/assets target/cooked/examples
cargo run --example scene
```

The Windows artifact holds the sample cooked for `scene.exe`, in `assets` beside it.

The log level of every example is set by the `SLIME_LOG` environment variable, for example
`SLIME_LOG=debug cargo run --example hello`.

## Shared code and tests

- [`common/`](common) holds code used by several examples and tests, each file included where it is
  needed with `#[path = "common/<file>.rs"] mod <file>;`: the triangle that `window`, `offscreen`
  and its image test draw, and its shader; the sample scene, a cooked scene loaded with its sun, sky
  and exposure in physical units (`sample_scene.rs`) and the camera placed around a point
  (`orbit.rs`), which `scene` and its image test share; the shapes (a cube, a sphere, a checker
  texture, six small spheres of increasing roughness, dielectric and metal, their materials, light
  and sky) of another image test; PNG writing; and the comparison with a reference image.
- [`assets/`](assets/README.md) holds the sample scene, the Blender script that made it, and its
  license.
- The image tests compare a render with a reference, within a tolerance
  ([ADR-0018](../docs/adr/0018-reference-image-tests.md), amended by
  [ADR-0028](../docs/adr/0028-image-tests-tolerate-a-few-pixels.md) and
  [ADR-0030](../docs/adr/0030-image-tests-strict-on-the-reference-driver.md)):
  [`tests/offscreen.rs`](tests/offscreen.rs) renders the triangle
  ([`tests/reference/triangle.png`](tests/reference/triangle.png)),
  [`tests/scene.rs`](tests/scene.rs) the shapes at a fixed time
  ([`tests/reference/shapes.png`](tests/reference/shapes.png)), and [`tests/gltf.rs`](tests/gltf.rs)
  the sample scene from two camera poses
  ([`tests/reference/gltf-front.png`](tests/reference/gltf-front.png) and
  [`tests/reference/gltf-back.png`](tests/reference/gltf-back.png)). Their module comments explain
  how to render a reference again after a deliberate change; the tests print the difference they
  measured with `--nocapture`. `tests/gltf.rs` cooks the sample's sources before it draws them, and
  checks that the textures of the sample's boxes stand upright on every side. It also builds small
  GLB files in memory with the `gltf` crate, cooks them and draws them: a mirrored node, an
  emission strength, masked, blended and double-sided materials, a file's own tangents, a texture
  read both as colors and as data, and the lights a file places; and it moves and renames a cooked
  `.gltf` file, which changes nothing it draws. What the importer refuses is tested with it, in
  [`crates/slime-cook/tests/gltf.rs`](../crates/slime-cook/tests/gltf.rs). The unit
  tests of `common/orbit.rs` run in `tests/gltf.rs` too.

To add an example, put its source in this directory, declare it as an `[[example]]` in
[`Cargo.toml`](Cargo.toml), give it a module comment saying what it shows and how to run it, and
add it to the table above; to publish it for Windows too, add it to the artifact's list in
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml) and to the list of executables above.
