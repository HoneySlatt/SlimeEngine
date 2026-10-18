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
| [`scene`](scene.rs) | A scene cooked by the asset pipeline ([ADR-0069](../docs/adr/0069-the-asset-pipeline.md)) and drawn by the renderer (`slime-render`): the asset of [`assets/scene.glb`](assets/README.md), cooked first (below), or any scene that `slime-cook` cooked, with its physically based materials, lit in physical units by the lights its nodes place (the asset's sun, the light of its glowing ring and a spot light) or, for a scene without lights, by the sample's sun, and by a clear blue sky around the first directional light, drawn behind the scene with the sun's disc (above what the camera can see in the asset, whose sun is high) and a bloom that makes bright light glow, seen from a camera that orbits it until the mouse takes over (drag with the left button to turn, from level with the scene to nearly straight down, roll the wheel to come nearer or farther), with a dev overlay showing the frame time, the CPU time of the render, the GPU time of each pass (shadow, scene, sky, bloom, tone mapping), the draw calls and triangles, the draw calls for shadows, the CPU time taken by the sky once the scene was loaded, an exposure slider in a photographer's exposure values, EV100, and a bloom strength slider, from none to five times the sample's, and, for the sample's asset, a slider of how strongly its ring glows, from none to four times, which changes the ring's material while the example runs, found by its name (`LoadedScene::material`, `Material::set_values`) (F1 hides it); Escape, closing the window or Ctrl+C quits cleanly. It loads the scene in the background (`Renderer::load_scene_in_background`), its frames showing the sky's horizon until the scene is loaded. The sample of Stages 3a, 3b, 4a and 4b | `--assets <directory>` reads the cooked data of another directory (by default, `assets` beside the program when it holds a manifest, as in the Windows artifact below, and `target/cooked/examples` otherwise); `--scene <id>` draws another of its scenes, by its ID (a glTF file's default scene has the file's ID); `--frames <count>` closes it after that many frames that drew the scene, and fails if none did; `--watch` loads the scene again, keeping what did not change, when its cooked data changes, beside `slime-cook watch` running on its sources ([ADR-0076](../docs/adr/0076-hot-reload-of-cooked-data.md)), and the renderer's shaders when their files change ([ADR-0077](../docs/adr/0077-hot-reload-of-shaders.md)) |
| [`world`](world.rs) | The examples' yard drawn from a world ([ADR-0084](../docs/adr/0084-what-the-world-draws.md)): a scene file, [`assets/yard.scene`](assets/yard.scene), that places the cooked sample scene and four lamps of [`assets/lamp.scene`](assets/lamp.scene), a prefab made of the sample's meshes, the second lamp red and the fourth without its light ([ADR-0095](../docs/adr/0095-cooked-scene-files.md)), spawned in the world that `slime-app` keeps through a library of the cooked scenes (`Context::world_mut`, `SceneLibrary::of`, `World::spawn_scene`), its meshes loaded by an asset cache in the background (`slime_render::AssetCache`), and drawn through `slime-world-render` (`FrameBuilder`), the file's first directional light showing in the sky as its sun; the glowing ring floats up and down at the fixed step, its light with it, and each frame draws it between its last two places (`World::interpolated_transform`); the camera turns around the scene; after a lost GPU device the cache loads the world's meshes again, and the world is untouched; F2 shows the dev overlay's world panel (`slime_devui::WorldPanel`), which lists the entities by name and edits their components, the example applying its edits to the world once each frame is drawn; Escape, closing the window or Ctrl+C quits, unless a text field of the panel has the keyboard or a menu of it is open, which Escape leaves or closes; when it quits, it saves its world, what differs from the world as it spawns it at the start ([ADR-0087](../docs/adr/0087-the-worlds-documents.md), `World::save`), loads the save into a world spawned anew (`World::load`), and fails if the two differ (`World::first_difference`); it also writes the yard back as a scene file (`World::write_scene`), spawns the file, and fails if the world spawned is not the one written; `SLIME_LOG=debug` shows the save and the file | `--assets <directory>` as for `scene`; `--frames <count>` closes it after that many frames that drew the whole world, and fails if none did, or, with `--headless`, after that many frames; `--headless` runs the world without a window: the ring floats, nothing is drawn, and the run fails if the ring did not float; `--watch` draws again what changes in the cooked data (`AssetCache::reload`), and carries the run into the yard spawned anew when a scene changed (`SceneLibrary::reload`, `World::carry`), with a window or without; a change that the world refuses is logged, and leaves the run as it is |

On Windows, the same commands work with the toolchain described in
[`CONTRIBUTING.md`](../CONTRIBUTING.md#on-windows). Each CI run on GitHub also publishes
`hello.exe`, `headless.exe`, `window.exe`, `offscreen.exe`, `scene.exe` and `world.exe` built for
Windows, as the artifact `slime-examples-windows`, with the game, `hunnawood.exe`
([`games/hunnawood`](../games/hunnawood/README.md)) and its cooked zone, to run on a Windows machine
without a Rust toolchain. They link the C runtime of the MSVC toolchain statically, so they start on a
fresh Windows install too, without the Visual C++ Redistributable. Run them from a terminal (Windows
Terminal, PowerShell or cmd): started by a double click, their console closes with them, and an
error at startup disappears with it.

The `scene` and `world` examples read cooked data, which the cooker (`slime-cook`,
[`crates/slime-cook`](../crates/slime-cook/README.md)) makes from the sources in
[`assets/`](assets/README.md). Cook them once, and again after a source changes:

```sh
cargo run -p slime-cook -- cook examples/assets target/cooked/examples
cargo run --example scene
cargo run --example world
```

The Windows artifact holds the sample cooked for `scene.exe` and `world.exe`, in `assets` beside
them.

The log level of every example is set by the `SLIME_LOG` environment variable, for example
`SLIME_LOG=debug cargo run --example hello`.

## Shared code and tests

- [`common/`](common) holds code used by several examples and tests, each file included where it is
  needed with `#[path = "common/<file>.rs"] mod <file>;`: the triangle that `window`, `offscreen`
  and its image test draw, and its shader; the sample scene, a cooked scene loaded with its sun, sky
  and exposure in physical units (`sample_scene.rs`) and the camera placed around a point
  (`orbit.rs`), which `scene` and its image test share; the yard's ID (`yard.rs`), which `world` and
  its tests share; the sample cooked once for the tests of
  one program, and a directory of its own for each test (`cooked.rs`); the shapes (a cube, a
  sphere, a checker texture, six small spheres of increasing roughness, dielectric and metal, their
  materials, light and sky) of another image test; PNG writing; and the comparison with a reference
  image.
- [`assets/`](assets/README.md) holds the sample scene, the Blender script that made it, the yard's
  and the lamp's scene files, and their licenses.
- The image tests compare a render with a reference, within a tolerance
  ([ADR-0018](../docs/adr/0018-reference-image-tests.md), amended by
  [ADR-0028](../docs/adr/0028-image-tests-tolerate-a-few-pixels.md),
  [ADR-0030](../docs/adr/0030-image-tests-strict-on-the-reference-driver.md) and
  [ADR-0054](../docs/adr/0054-image-tests-allow-0-2-percent-on-other-drivers.md)):
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
  `.gltf` file, which changes nothing it draws. It spawns the sample and a mirrored node in a world
  (`slime-world`), which places each node where its file does. What the importer refuses is tested
  with it, in [`crates/slime-cook/tests/gltf.rs`](../crates/slime-cook/tests/gltf.rs). The unit
  tests of `common/orbit.rs` run in `tests/gltf.rs` too.
- [`tests/world.rs`](tests/world.rs) spawns the sample in a world and draws it through
  `slime-world-render`, from the meshes of an asset cache: the instances that the scene loaded by
  the renderer gives, and the reference images of `tests/gltf.rs`, within the same tolerance; the
  ring moved at fixed steps, drawn where the fraction of a step puts it, and not sliding when
  teleported; a hidden subtree and its light left out; and, after a lost device, the same image
  drawn from the cache made again, the world untouched. Without a GPU, it spawns the cooked yard,
  its lamps placed with their overrides, and changes the lamp in a copy of the sources, cooks it
  again and carries a run into the yard spawned anew: every lamp follows but where the yard
  overrides it, and what the run changed stays.

To add an example, put its source in this directory, declare it as an `[[example]]` in
[`Cargo.toml`](Cargo.toml), give it a module comment saying what it shows and how to run it, and
add it to the table above; to publish it for Windows too, add it to the artifact's list in
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml) and to the list of executables above.
