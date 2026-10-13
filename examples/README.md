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
| [`scene`](scene.rs) | A glTF scene drawn by the renderer (`slime-render`): [`assets/scene.glb`](assets/README.md), or any GLB file that holds its buffer and PNG images in its binary chunk, with its physically based materials, lit by a directional light, seen from a camera that orbits it until the mouse takes over (drag with the left button to turn, roll the wheel to come nearer or farther), with a dev overlay showing the frame time, the CPU time of the render, the GPU time of each pass (scene, tone mapping), the draw calls and triangles, and an exposure slider (F1 hides it); Escape, closing the window or Ctrl+C quits cleanly. The sample of Stages 3a and 3b | `--scene <file.glb>` draws another file; `--frames <count>` closes it after that many frames, and fails if none of them drew the scene |

On Windows, the same commands work with the toolchain described in
[`CONTRIBUTING.md`](../CONTRIBUTING.md#on-windows). Each CI run on GitHub also publishes
`hello.exe`, `headless.exe`, `window.exe`, `offscreen.exe` and `scene.exe` built for Windows, as
the artifact `slime-examples-windows`, to run on a Windows machine without a Rust toolchain. They
link the C runtime of the MSVC toolchain dynamically, so they need `VCRUNTIME140.dll`, which the
x64 Microsoft Visual C++ Redistributable installs: most Windows machines have it already, but a
fresh install or Windows Sandbox does not, and the programs then fail to start with a system error
naming that file. Run them from a terminal (Windows Terminal, PowerShell or cmd): started by a
double click, their console closes with them, and an error at startup disappears with it.

The log level of every example is set by the `SLIME_LOG` environment variable, for example
`SLIME_LOG=debug cargo run --example hello`.

## Shared code and tests

- [`common/`](common) holds code used by several examples and tests, each file included where it is
  needed with `#[path = "common/<file>.rs"] mod <file>;`: the triangle that `window`, `offscreen`
  and its image test draw, and its shader; the temporary glTF loader ([`gltf.rs`](common/gltf.rs),
  [ADR-0029](../docs/adr/0029-temporary-gltf-loading.md),
  [ADR-0035](../docs/adr/0035-physically-based-materials.md)); the sample scene with its asset and its
  light (`sample_scene.rs`) and the camera placed around a point (`orbit.rs`), which `scene` and its
  image test share; the shapes (a cube, a sphere, a checker texture, six small spheres of increasing
  roughness, dielectric and metal, their materials and light) of another image test; PNG writing;
  the comparison with a reference image; error messages with their causes.
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
  measured with `--nocapture`. `tests/gltf.rs` also gives the glTF loader small GLB files built in
  memory, to check what it refuses, how it draws a mirrored node, the extensions a file requires,
  masked, blended and double-sided materials, a file's own tangents and a texture read both as
  colors and as data, and that it places a node hierarchy a hundred thousand levels deep, and runs
  the unit tests of `common/gltf.rs` and `common/orbit.rs`.

To add an example, put its source in this directory, declare it as an `[[example]]` in
[`Cargo.toml`](Cargo.toml), give it a module comment saying what it shows and how to run it, and
add it to the table above; to publish it for Windows too, add it to the artifact's list in
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml) and to the list of executables above.
