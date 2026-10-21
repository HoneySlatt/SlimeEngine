# slime-world-render

The bridge from a world to the renderer: what a world (`slime-world`) draws, built at each frame for
the renderer (`slime-render`) from the GPU objects of an asset cache
(`docs/adr/0084-what-the-world-draws.md`).

## Status

Started in M5a.3 (`docs/PLAN.md`). It draws the meshes and the lights of a world's entities where
the world draws them between fixed steps, leaves out what is hidden and what the cache does not
hold yet, and draws the game's sky around the world's sun.

## Role

- Builds, at each frame, the instances of the meshes that a world's entities place, from the GPU
  objects of a `slime_render::AssetCache`, which it asks for the meshes it does not hold; the
  lights they place, but those that the renderer would not draw where their entities' transforms,
  composed, place them (`Light::check`), which it leaves out with a warning; and the sky's
  environment around the directional light that shows in the sky, given once to the sky and to
  the lights.
- Keeps no world and no mesh: the world keeps what is drawn, the cache its GPU objects. It keeps the
  sky's environment, which it makes again when the sky or its sun changes.

## Entry points

```rust,no_run
use slime_gfx::Gpu;
use slime_render::{AssetCache, Camera, RenderError, Renderer};
use slime_world::World;
use slime_world_render::FrameBuilder;

// At each frame, after the world's fixed steps: the cache loads what the world names, and the
// frame draws what it holds already.
fn draw(
    builder: &mut FrameBuilder,
    world: &World,
    cache: &mut AssetCache,
    renderer: &mut Renderer,
    gpu: &Gpu,
    target: &slime_gfx::RenderTarget,
    alpha: f64,
) -> Result<(), RenderError> {
    let frame = builder.build(world, cache, gpu, alpha)?;
    let mut scene = frame.scene(Camera::default());
    scene.exposure = 0.5;
    renderer.render(gpu, target, &scene)?;
    Ok(())
}
```

- `FrameBuilder::new(sky)` draws a world around the game's `Sky`, without its sun, which the world
  gives; `FrameBuilder::build(world, cache, gpu, alpha)` gives a `Frame`, the instances, lights and
  environment of one frame, which it owns, so that it can be drawn on another thread, and whose
  `scene(camera)` is the renderer's `Scene`, its lights
  the nearest to the camera first when they are more than the renderer draws, with how many meshes
  still load and why the batches that finished failed.
- `FrameBuilder::set_sky` draws the world around another sky from the next frame on, and
  `FrameBuilder::release` drops the environment when the device is lost.

The rustdoc holds the contracts: what is drawn, and what building a frame costs.

## Dependencies

- `slime-world`: the world it reads.
- `slime-render`: the renderer's instances, lights and sky, and the asset cache.
- `slime-gfx`: the GPU the cache loads onto and the environment is made on.
- `tracing`: the errors of the meshes that cannot be loaded, and the warning of several suns.

## Must not

- Keep anything of a world between frames: the world is the source of truth.
- Be a dependency of `slime-world` or `slime-render`: neither knows the other (ADR-0079).
