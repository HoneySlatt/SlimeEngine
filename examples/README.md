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
| [`headless`](headless.rs) | A game's rules without a window, a simulation (`slime_app::Simulation`): frames back to back with simulated time, exit at a frame limit, profiling spans (`SLIME_LOG=trace`) | `--frames <count>` (default 600) |
| [`window`](window.rs) | A window in which the GPU draws the samples' triangle, which follows resizes, a presentation without rules (`slime_app::Presentation`); its events logged (window, keys, mouse; cursor motion with `SLIME_LOG=debug`); Escape, closing it or Ctrl+C quits cleanly. The sample of Stage 2 | `--frames <count>` closes it after that many frames and, with a window, fails if none of them drew the triangle; `--headless` runs the same program's frames without a window, its presentation not called; `--present-mode mailbox|fifo` chooses how images are shown (default `mailbox`) |
| [`offscreen`](offscreen.rs) | Rendering without a window: the GPU draws a triangle into an image, which is read back and written to a PNG file; works on software drivers (lavapipe, WARP) | `--output <file>` (default `triangle.png`) |
| [`scene`](scene.rs) | A scene cooked by the asset pipeline ([ADR-0069](../docs/adr/0069-the-asset-pipeline.md)) and drawn by the renderer (`slime-render`): the asset of [`assets/scene.glb`](assets/README.md), cooked first (below), or any scene that `slime-cook` cooked, with its physically based materials, lit in physical units by the lights its nodes place (the asset's sun, the light of its glowing ring and a spot light) or, for a scene without lights, by the sample's sun, and by a clear blue sky around the first directional light, drawn behind the scene with the sun's disc (above what the camera can see in the asset, whose sun is high) and a bloom that makes bright light glow, seen from a camera that orbits it until the mouse takes over (drag with the left button to turn, from level with the scene to nearly straight down, roll the wheel to come nearer or farther), with a dev overlay showing the frame time, the CPU time of the render, the GPU time of each pass (shadow, scene, sky, bloom, tone mapping), the draw calls, the instances that they draw and their triangles, the draw calls for shadows, the CPU time taken by the sky once the scene was loaded, an exposure slider in a photographer's exposure values, EV100, and a bloom strength slider, from none to five times the sample's, and, for the sample's asset, a slider of how strongly its ring glows, from none to four times, which changes the ring's material while the example runs, found by its name (`LoadedScene::material`, `Material::set_values`) (F1 hides it); Escape, closing the window or Ctrl+C quits cleanly. It loads the scene in the background (`Renderer::load_scene_in_background`), its frames showing the sky's horizon until the scene is loaded. The sample of Stages 3a, 3b, 4a and 4b | `--assets <directory>` reads the cooked data of another directory (by default, `assets` beside the program when it holds a manifest, as in the Windows artifact below, and `target/cooked/examples` otherwise); `--scene <id>` draws another of its scenes, by its ID (a glTF file's default scene has the file's ID); `--frames <count>` closes it after that many frames that drew the scene, and fails if none did; `--watch` loads the scene again, keeping what did not change, when its cooked data changes, beside `slime-cook watch` running on its sources ([ADR-0076](../docs/adr/0076-hot-reload-of-cooked-data.md)), and the renderer's shaders when their files change ([ADR-0077](../docs/adr/0077-hot-reload-of-shaders.md)) |
| [`world`](world.rs) | The examples' yard drawn from a world ([ADR-0084](../docs/adr/0084-what-the-world-draws.md)): a scene file, [`assets/yard.scene`](assets/yard.scene), that places the cooked sample scene and four lamps of [`assets/lamp.scene`](assets/lamp.scene), a prefab made of the sample's meshes, the second lamp red and the fourth without its light ([ADR-0095](../docs/adr/0095-cooked-scene-files.md)), spawned by its rules, a simulation, in the world that `slime-app`'s host keeps, through a library of the cooked scenes (`Simulation::start`, `SceneLibrary::of`, `World::spawn_scene`), and shown by a presentation, which reads the world and never writes it, its meshes asked for and loaded by an asset cache in the background, held while the world names them (`slime_render::AssetCache`, [ADR-0128](../docs/adr/0128-asset-requests.md)), and drawn through `slime-world-render` (`FrameBuilder`), the file's first directional light showing in the sky as its sun; the glowing ring floats up and down at the fixed step, its light with it, and each frame draws it between its last two places (`World::interpolated_transform`), its transform driven, as the yard marks it (`slime_world::Driven`), so that saves and the yard written back keep it where the file puts it; the camera turns around the scene, and a second view, orthographic, draws a map of the yard from above in the window's top right corner ([ADR-0126](../docs/adr/0126-views-apart-from-the-renderer.md), `Projection::Orthographic`, `ViewDraw::viewport`), the two views named `yard` and `map` for the tools that capture one (`Presentation::views`) and change their settings (`Presentation::view_settings`), and two reports that the tools read (`Presentation::reports`), `assets`, the cache's requests and memory against its budgets, and `views`, the views' statistics; after a lost GPU device the cache made again loads the meshes that the frame builder's handles hold (`AssetCache::into_requests`), and the world is untouched, the views made again with their settings; F2 shows the dev overlay's world panel (`slime_devui::WorldPanel`), which lists the entities by name and edits their components, the example sending its edits as commands, which the host applies at the start of the next frame (`slime_world::Command`); F3 its panel of the views, which shows the size each draws at and edits its settings, its render scale, anti-aliasing, a mode of debugging that shows normals or shadows, and whether it draws the shadows, the sky and the bloom ([ADR-0127](../docs/adr/0127-the-settings-of-a-view.md), `slime_render::ViewSettings`, `slime_devui::edit_reflected`); a window's edge dragged stretches the views' images until it is let go, when they make their targets at the new size; Escape, closing the window or Ctrl+C quits, unless a text field of the panel has the keyboard or a menu of it is open, which Escape leaves or closes; when it quits, it saves its world, what the run changed since the example settled the world that it spawns at the start ([ADR-0125](../docs/adr/0125-saves-per-scene.md), `World::settle`, `World::save`), loads the save into a world spawned anew (`World::load`), and fails if the two differ (`World::first_difference`); it also writes the yard back as a scene file (`World::write_scene`), spawns the file, and fails if the world spawned is not the one written; `SLIME_LOG=debug` shows the save and the file | `--assets <directory>` as for `scene`; `--frames <count>` closes it after that many frames that drew the whole world, and fails if none did, or, with `--headless`, after that many frames; `--headless` runs the world without a window: the ring floats, nothing is drawn, and the run fails if the ring did not float; `--watch` draws again what changes in the cooked data (`AssetCache::reload`), and carries the run into the yard spawned anew when a scene changed (`SceneLibrary::reload`, `World::carry`), with a window or without; a change that the world refuses is logged, and leaves the run as it is |

The examples that run through `slime_app::run` hold the development channel, `slime-app`'s
`development` feature ([ADR-0122](../docs/adr/0122-the-development-channel.md)): started with a
token, as the engine's MCP server ([`crates/slime-mcp`](../crates/slime-mcp/README.md)) starts a
program, they listen for the tools that drive them; started without one, they do not.

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
  drawn from the cache made again with the lost one's requests, and from one made without them,
  the world untouched; a frame built on one thread drawn on
  another, once its cache and builder are dropped, as on the first. Without a GPU, it spawns the
  cooked yard, its lamps placed with their overrides, their lights placed in the world, and changes
  the lamp in a copy of the sources, cooks it
  again and carries a run into the yard spawned anew: every lamp follows but where the yard
  overrides it, what the run changed stays, and the ring, whose transform is driven, comes back
  where the yard puts it.

To add an example, put its source in this directory, declare it as an `[[example]]` in
[`Cargo.toml`](Cargo.toml), give it a module comment saying what it shows and how to run it, and
add it to the table above; to publish it for Windows too, add it to the artifact's list in
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml) and to the list of executables above.
