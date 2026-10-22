# slime-world-render

The bridge from a world to the renderer: what a world (`slime-world`) draws, built at each frame for
the renderer (`slime-render`) from the GPU objects of an asset cache
(`docs/adr/0084-what-the-world-draws.md`).

## Status

Started in M5a.3 (`docs/PLAN.md`). It draws the meshes and the lights of a world's entities where
the world draws them between fixed steps, leaves out what is hidden and what the cache does not
hold yet, and draws the game's sky around the world's sun. It numbers each instance with its
entity, which picking by the GPU gives back, and picks by the CPU, a ray against the meshes that a
world places (`docs/adr/0130-picking.md`).

## Role

- Builds, at each frame, the instances of the meshes that a world's entities place, from the GPU
  objects of a `slime_render::AssetCache`, which it asks for the meshes it holds no handle of, at
  priority now, keeping a handle of each mesh that the world names, until the world no longer
  names it (`docs/adr/0128-asset-requests.md`); the
  lights they place, but those that the renderer would not draw where their entities' transforms,
  composed, place them (`Light::check`), which it leaves out with a warning; and the sky's
  environment around the directional light that shows in the sky, given once to the sky and to
  the lights.
- Keeps no world and no mesh on the GPU: the world keeps what is drawn, the cache its GPU objects.
  It keeps the sky's environment, which it makes again when the sky or its sun changes, and the
  handles of the meshes the world names, which hold nothing of the GPU's; given a cache that did not
  issue them, as one made again without the requests of a lost device's, it asks that cache again.
- Numbers each instance with its entity, for picking by the GPU, and picks by the CPU: its `Picker`
  keeps in the CPU's memory the boxes and triangles of the meshes that the world names, read from
  cooked data (`docs/adr/0130-picking.md`).

## Entry points

```rust,no_run
use slime_gfx::Gpu;
use slime_render::{AssetCache, Camera, RenderError, Renderer, View, ViewDraw};
use slime_world::World;
use slime_world_render::FrameBuilder;

// At each frame, after the world's fixed steps: the cache loads what the world names, and the
// frame draws what it holds already.
fn draw(
    builder: &mut FrameBuilder,
    world: &World,
    cache: &mut AssetCache,
    renderer: &mut Renderer,
    view: &mut View,
    gpu: &Gpu,
    target: &slime_gfx::RenderTarget,
    alpha: f64,
) -> Result<(), RenderError> {
    let frame = builder.build(world, cache, gpu, alpha)?;
    let camera = Camera::default();
    // The lamps nearest the camera are drawn when the world places more than the renderer draws.
    let mut scene = frame.scene(camera.eye);
    scene.exposure = 0.5;
    renderer.render(gpu, &scene, &mut [ViewDraw::new(view, camera, target)])?;
    Ok(())
}
```

- `FrameBuilder::new(sky)` draws a world around the game's `Sky`, without its sun, which the world
  gives; `FrameBuilder::build(world, cache, gpu, alpha)` gives a `Frame`, the instances, lights and
  environment of one frame, which it owns, so that it can be drawn on another thread, each light
  with its entity, so that a game's presentation changes what it shows of one without the world, and
  whose `scene(near)` is the renderer's `Scene`, its lights the nearest to a point that the game
  gives first when they are more than the renderer draws, with how many meshes still load and each
  mesh that failed since the last frame, with why.
- `FrameBuilder::set_sky` draws the world around another sky from the next frame on, and
  `FrameBuilder::release` drops the environment when the device is lost.
- Each instance of a frame is picked as its entity: its `Instance::pick` is the entity's handle,
  which `Entity::from_bits` gives back from what a view's picking found.
- `Picker::new(assets)` picks by the CPU, without a GPU: `cast(world, alpha, ray)` gives the entity
  whose mesh a ray meets first (`Hit`), the ray that `Camera::ray` gives through a pixel, reading
  the meshes from the cooked data at its first cast in a world that names them, and keeping their
  boxes, and the triangles of those that rays meet, while the world names them.

The rustdoc holds the contracts: what is drawn, and what building a frame costs.

## Dependencies

- `slime-world`: the world it reads.
- `slime-render`: the renderer's instances, lights and sky, and the asset cache.
- `slime-gfx`: the GPU the cache loads onto and the environment is made on.
- `slime-assets`: the IDs of the meshes that its handles hold, and the cooked meshes that picking
  by the CPU reads.
- `tracing`: the errors of the meshes that cannot be loaded, and the warning of several suns.

## Must not

- Keep a copy of a world's state between frames: the world is the source of truth. What it keeps,
  the sky's environment, the handles of the meshes that the world names, the entities whose lights
  it warned of and the picker's meshes, holds none of the world's values.
- Be a dependency of `slime-world` or `slime-render`: neither knows the other (ADR-0079).
