# slime-render

The renderer: draws meshes with their materials, seen from a camera, into render targets, through
the GPU layer (`slime-gfx`).

## Status

Partial (`docs/PLAN.md`). It draws instances of meshes from a perspective camera, each with a
material in glTF's metallic-roughness model, lit by directional, point and spot lights in physical
units and by a procedural sky that surfaces reflect and that shows behind them, with the shadows of
one directional light around the camera, drawn in four cascades, and of every spot light that casts
them. It draws in high dynamic range, then
exposes the light, spreads a bloom around it, tone maps it and dithers it into the output, and
reports what each frame cost on the CPU and the GPU. It smooths the edges of what it draws with four
samples per pixel. It loads cooked scenes in the background, and again when their cooked data
changes, and keeps the meshes that a game draws by their asset IDs in a cache; a material's values
change while a game runs, and so do the renderer's shaders. It does not yet blend transparent
surfaces or leave out what the camera does not see.

## Role

- Owns what drawing a scene needs beyond the GPU layer: the vertex layout and the shaders, the
  camera's matrices, the values the shaders read (per frame, per material, per instance) and
  their layout, the high-dynamic-range and depth targets, and the buffer of the instances'
  values.
- Orders and records its frame's passes with a render graph (`src/graph.rs`, private;
  `docs/adr/0041-render-graph.md`): each pass declares the targets it draws into and the ones it
  samples, the graph orders the passes by them, creates the targets that live for one frame at the
  output's size or a fraction of it, and records the passes in order, four to a submission. The
  graph is the renderer's own: games that draw with `slime-gfx` order their passes themselves.
- Is built on `slime-gfx`'s engine-owned types only: it does not use wgpu
  (`docs/adr/0027-the-render-crate.md`).
- Loads cooked scenes on threads of its own: a thread per load, and a task pool private to the
  crate (`src/pool.rs`), whose threads read and upload one mesh or one texture per task through
  clones of the `Gpu` (`docs/adr/0073-loading-scenes-in-the-background.md`).
- Draws what it is given: a flat list of instances that the application builds for each frame.
  It keeps no scene between frames and has no scene graph; `slime-world-render` builds such lists
  from a world (`docs/adr/0084-what-the-world-draws.md`).

## Entry points

Drawing a red triangle into an offscreen image:

```rust
use slime_color::LinearColor;
use slime_gfx::{Gpu, TextureFormat};
use slime_render::glam::{DAffine3, DVec3, Vec3};
use slime_render::{
    Camera, DirectionalLight, Instance, Light, MaterialData, MeshData, RenderError, Renderer, Scene,
};

use std::f32::consts::PI;

fn main() -> Result<(), RenderError> {
    let gpu = Gpu::new()?;
    let target = gpu.create_render_target("image", 64, 64, TextureFormat::Rgba8UnormSrgb)?;
    let mut renderer = Renderer::new(&gpu, target.format())?;
    let triangle = renderer.create_mesh(&gpu, "triangle", &MeshData {
        positions: vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 1.0], [1.0, 1.0], [0.5, 0.0]],
        // Along +X, where u grows.
        tangents: vec![[1.0, 0.0, 0.0, 1.0]; 3],
        indices: vec![0, 1, 2],
    })?;
    // The default material is a white matte dielectric.
    let red = renderer.create_material(
        &gpu,
        "red",
        &MaterialData { base_color: LinearColor::rgb(1.0, 0.0, 0.0), ..MaterialData::default() },
    )?;

    let camera = Camera { eye: DVec3::new(0.0, 0.0, 3.0), ..Camera::default() };
    // The sun of a clear day, 100 000 lux, shining straight at the triangle, which faces +Z, and no
    // environment (a black sky): the matte red surface reflects 0.97 / π of it in red and
    // 0.01 / π in green and blue, about 30 900 and 320 nits, which an exposure of π / 100 000
    // brings to 0.97 and 0.01.
    let sun = Light::Directional(DirectionalLight {
        direction: Vec3::new(0.0, 0.0, -1.0),
        color: LinearColor::WHITE,
        illuminance: 100_000.0,
        ..DirectionalLight::default()
    });
    let instances = [Instance::new(&triangle, &red, DAffine3::IDENTITY)];
    let lights = [sun];
    // A scene starts without lights, environment or bloom (which would spread a little of the
    // triangle's light around its edges); its fields are then set.
    let mut scene = Scene::new(camera, &instances);
    scene.lights = &lights;
    scene.exposure = PI / 100_000.0;
    let stats = renderer.render(&gpu, &target, &scene)?;
    assert_eq!((stats.draw_calls, stats.triangles), (1, 1));

    let image = gpu.read_render_target(&target)?;
    let center = (32 * 64 + 32) * 4;
    // Tone mapping compresses light towards 1 and turns it towards white: (0.97, 0.01, 0.01)
    // shows as about (240, 29, 29), not (255, 0, 0).
    let [red, green, blue] = [0, 1, 2].map(|channel| image.pixels[center + channel]);
    assert!(red.abs_diff(240) <= 1 && green.abs_diff(29) <= 1 && blue.abs_diff(29) <= 1);
    Ok(())
}
```

- `Renderer::new(gpu, format)` creates a renderer for output targets of one format: the window's
  for a windowed application (`Context::surface_format` in `slime-app`). Use one renderer per output
  size.
- `Renderer::set_shaders(gpu, &ShaderSources)` makes the renderer's pipelines again from new WGSL
  sources, all or none, so that a game sees a shader change while it runs; `ShaderSources` holds
  the embedded sources or those of the shaders' files, and `ShaderWatch` tells a game when the
  files change (ADR-0077).
- `Renderer::create_environment(gpu, &Sky)` computes, on the CPU, the `Environment` that lights a
  scene and shows behind it: a procedural sky of a zenith, a horizon, a ground and an optional sun.
  It is slow: create one per sky, not per frame.
- `MeshData::cube()`, `MeshData::sphere(rings, segments)` and `MeshData::square()` make simple
  shapes in code, one metre across and centred on their origin, for an instance's transform to
  place and scale.
- `Renderer::create_mesh(gpu, label, &MeshData)` uploads a `Mesh`; mesh data that cannot be drawn
  is returned as `RenderError::InvalidMesh`. `Renderer::create_material(gpu, label,
  &MaterialData)` creates a `Material` with glTF's metallic-roughness model, its textures given as
  `SampledTexture`s; values out of their ranges are returned as `RenderError::InvalidMaterial`.
  `Material::set_values(gpu, MaterialValues)` changes a material's values while a game runs, its
  textures staying as made, for a fire that flickers; `Material::values` reads them. Meshes,
  materials and environments are shared by their clones.
- `Renderer::create_texture(gpu, label, &CookedTexture)` uploads a texture cooked by the asset
  pipeline, read from a directory of cooked data with `slime_assets::CookedAssets::texture`: BC7,
  BC5 or 8-bit texels, every level as the cooker made it (ADR-0071). Normal textures are read by
  their x and y, z rebuilt, so that BC5 serves.
- `Renderer::load_scene_in_background(gpu, &CookedAssets, id)` starts loading a cooked scene onto
  the GPU, on the renderer's threads, and returns a `SceneLoad` at once, which a game polls at each
  frame with `take` until it gives the scene, whole; `Renderer::load_scene` starts a load and waits
  for it. A load reads every mesh its nodes place, the materials they are drawn with and the
  textures those read, each once; its `LoadedScene` gives the `instances` to draw, the `lights` in
  world space, the box around them, its materials by their names (`material`), and the cooked
  scene itself, whose nodes carry their `extras` and their typed components (`SceneNode`). What
  cannot be loaded is a `SceneError`.
  `Renderer::reload_scene_in_background(gpu, &CookedAssets, id, &previous)` loads a scene again
  once its cooked data changed, keeping the meshes and textures of the scene loaded before whose
  cooked file did not change (ADR-0076).
- `AssetCache::new(CookedAssets)` keeps the meshes that a game draws by their asset IDs, with the
  materials they are drawn with and the textures those read, so that a world names them by ID and
  holds no GPU object (ADR-0084): `request(renderer, gpu, ids)` starts loading, as one batch in the
  background, the meshes it neither holds nor loads; `poll` takes the batches that finished, each
  whole, and gives the errors of those that failed; `mesh(id)` gives a mesh and its material once
  held, `material(id)` a material, `loading` how many meshes are on their way.
  `reload(renderer, gpu, assets)` loads again what changed in new cooked data, keeping the rest,
  `reloading` says whether it is under way, and `clear` drops everything after a lost device.
- `scene_lights(&CookedScene)` gives the lights that a cooked scene's nodes place, as a
  `LoadedScene` holds them, without loading the scene: a game knows its sun before its scene is
  loaded.
- `Scene::new(camera, &instances)` and `Instance::new(mesh, material, transform)` make what a
  frame draws; their other fields, such as the scene's `lights`, `environment`, `exposure` and
  `bloom`, or whether an instance casts shadows, are set once they are made (ADR-0046).
- `Renderer::render(gpu, target, &scene)` draws a frame in passes (the directional light's shadows,
  the spot lights' shadows, scene, sky, resolve, bloom, tone mapping) and returns its `RenderStats`:
  draw calls, triangles, and the CPU and GPU time of its passes.
- `Camera`: where the scene is seen from. `Light`: one of glTF's punctual lights, a
  `DirectionalLight`, of which the first that casts shadows casts the scene's through cascades, a
  `PointLight` or a `SpotLight`, every one of which may cast shadows into a tile of the shadow
  atlas; a scene has at most `MAX_LIGHTS` of them drawn.
- `RenderError`: an invalid mesh or material, or a failure of the GPU layer (`GfxError`): a shader
  or pipeline the driver rejects, memory that runs out, or a lost device.
- `slime_render::glam` re-exports the math crate the API uses (`DVec3`, `DAffine3`, `Vec3`), so
  that callers use the same version.

The documentation of each item holds its contracts: what it draws, its limits, its costs and its
errors.

## Conventions

- World space is right-handed with +Y up, as in glTF; the camera looks down its −Z axis; front
  faces wind counter-clockwise, and an instance that mirrors its mesh keeps its front faces.
- Distances are in metres, and light in physical units, as in glTF: lux, candelas, and nits for the
  light that surfaces reflect and emit and for the sky.
- Colors are `slime-color`'s `LinearColor`s, linear (ADR-0045). Light is not negative and may
  exceed 1 until it is tone mapped.
- Depth is reversed, from 1 at the near plane to 0 at infinity (ADR-0047).
- Tangents and normal textures follow glTF.
- Positions are world positions, as `f64`s (ADR-0079): the camera's, the lights' and the
  instances'; the renderer draws them relative to the camera, in `f64` until they reach the GPU
  as `f32`s (ADR-0048).

## Dependencies

- `slime-gfx`: the GPU objects and the render passes.
- `slime-color`: `LinearColor`, the colors of materials, lights and skies.
- `slime-assets`: the cooked data of the asset pipeline, which the renderer turns into its own GPU
  objects (ADR-0069).
- `glam`: vectors, matrices and quaternions.
- `thiserror`: derives `RenderError`'s `Display` and `Error` implementations.
- `tracing`: logging.

## Must not

- Use wgpu: it draws through `slime-gfx`'s types only.
- Depend on `slime-platform` or `slime-app`: it draws into any render target, with or without a
  window.
- Keep a scene between frames: it draws what it is given.
