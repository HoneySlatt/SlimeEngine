//! The examples' cooked scene drawn from a world (`docs/adr/0084-what-the-world-draws.md`): spawned
//! in a `slime-world` world, its meshes loaded by a `slime-render` asset cache, and drawn through
//! `slime-world-render`. It draws the instances that the scene loaded by the renderer gives, and the
//! glTF test's reference images (`tests/reference/gltf-front.png` and `gltf-back.png`) within the
//! same tolerance (`common/reference_image.rs`); what moves at fixed steps is drawn where the
//! fraction of a step puts it; a hidden subtree is not drawn, nor its light; after a lost device,
//! the cache made again draws the same world, which the loss left as it was; a frame built on one
//! thread, once its cache and builder are dropped, is drawn on another as on the first. The cooked
//! yard (`examples/assets/yard.scene`) spawns its lamps with their overrides, and follows its lamp
//! cooked again, without a GPU (`docs/adr/0095-cooked-scene-files.md`). Runs on whatever adapter
//! the machine offers.

#[path = "../common/cooked.rs"]
mod cooked;
#[path = "../common/orbit.rs"]
mod orbit;
#[path = "../common/reference_image.rs"]
#[expect(
    dead_code,
    reason = "these tests compare with the glTF test's references, which that test renders again"
)]
mod reference_image;
#[path = "../common/sample_scene.rs"]
#[expect(
    dead_code,
    reason = "these tests take the sample's constants and its box, not its `SampleScene`"
)]
mod sample_scene;
#[path = "../common/yard.rs"]
mod yard;

use std::fs;
use std::path::Path;
use std::time::Duration;

use cooked::sample;
use sample_scene::{BLOOM, Bounds, EXPOSURE, SCENE, SKY};
use slime_gfx::{Gpu, Image, RenderTarget, TextureFormat};
use slime_render::glam::DAffine3;
use slime_render::{AssetCache, Camera, Light, RenderStats, Renderer};
use slime_world::{
    Entity, Hidden, Meshes, PersistentId, ResolvedScene, SceneLibrary, Transform, World,
};
use slime_world_render::FrameBuilder;
use yard::YARD;

const SIZE: u32 = 256;

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

/// A world that holds the sample, spawned under the key `sample`, its sun showing in the sky as
/// `SampleScene` gives it to its sky.
fn world() -> World {
    let scene = sample().scene(SCENE).expect("the sample's scene is read");
    let mut world = World::new();
    world
        .spawn_scene(
            "sample",
            &ResolvedScene::from_cooked(SCENE, &scene, world.registry()).expect("resolved"),
        )
        .expect("spawned");
    let sun = entity(&world, "sun");
    if let Some(mut light) = world.get_mut::<slime_world::Light>(sun)
        && let slime_world::Light::Directional { in_sky, .. } = &mut *light
    {
        *in_sky = true;
    }
    world.update_transforms();
    world
}

/// The entity of the sample's node `name`.
fn entity(world: &World, name: &str) -> Entity {
    world
        .find(&PersistentId::new(["sample", name]))
        .unwrap_or_else(|| panic!("the sample has a node named {name}"))
}

/// What draws a world: its renderer, the cache of its meshes, and what builds its frames.
struct Drawing {
    renderer: Renderer,
    cache: AssetCache,
    builder: FrameBuilder,
}

impl Drawing {
    fn new(gpu: &Gpu) -> Self {
        let renderer = Renderer::new(gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
        Self {
            cache: AssetCache::new(&renderer, sample().clone()),
            renderer,
            builder: FrameBuilder::new(Some(SKY)),
        }
    }

    /// Builds frames of `world` until the cache holds every mesh it names.
    fn settle(&mut self, gpu: &Gpu, world: &World) {
        for _ in 0..10_000 {
            let frame = self
                .builder
                .build(world, &mut self.cache, gpu, 0.0)
                .expect("the frame is built");
            assert!(frame.errors.is_empty(), "{:?}", frame.errors);
            if self.cache.loading() == 0 {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("the cache still loads after 10 s");
    }

    /// Draws `world` seen from `camera` at `fraction` of a step, as the sample is drawn, once the
    /// cache holds everything.
    fn draw(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        world: &World,
        camera: Camera,
    ) -> (Image, RenderStats) {
        self.settle(gpu, world);
        let frame = self
            .builder
            .build(world, &mut self.cache, gpu, 0.0)
            .expect("the frame is built");
        let mut scene = frame.scene(camera);
        scene.exposure = EXPOSURE;
        scene.bloom = BLOOM;
        let stats = self
            .renderer
            .render(gpu, target, &scene)
            .expect("the frame is drawn");
        let image = gpu.read_render_target(target).expect("read back");
        (image, stats)
    }
}

/// The box around the sample, as the renderer loads it, and the instances it gives.
fn loaded(gpu: &Gpu) -> (Bounds, Vec<DAffine3>) {
    let renderer = Renderer::new(gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let loaded = renderer
        .load_scene(gpu, sample(), SCENE)
        .expect("the scene loads");
    let (min, max) = loaded.bounds.expect("the sample places meshes");
    let transforms = loaded
        .instances()
        .iter()
        .map(|instance| instance.transform)
        .collect();
    (Bounds { min, max }, transforms)
}

/// The camera of the glTF test's reference image at `yaw` and `pitch`, and the shadow distance the
/// sample gives its sun for it.
fn camera(bounds: &Bounds, yaw: f32, pitch: f32) -> (Camera, f32) {
    let radius = bounds.radius();
    let camera = orbit::camera_at(bounds.center(), radius, radius * 2.2, yaw, pitch);
    let reach = camera.eye.distance(bounds.center()) as f32 + radius;
    (camera, reach)
}

/// Gives every directional light of `world` the shadow distance `reach`, as the sample does.
fn reach(world: &mut World, reach: f32) {
    world.query_mut::<&mut slime_world::Light>(|_, mut light| {
        if let slime_world::Light::Directional {
            shadow_distance, ..
        } = &mut *light
        {
            *shadow_distance = Some(reach);
        }
    });
}

fn target(gpu: &Gpu) -> RenderTarget {
    gpu.create_render_target("world", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it")
}

#[test]
fn the_sample_drawn_from_a_world_is_the_sample_the_renderer_loads() {
    let gpu = gpu();
    let target = target(&gpu);
    let (bounds, transforms) = loaded(&gpu);
    let mut world = world();
    let mut drawing = Drawing::new(&gpu);
    drawing.settle(&gpu, &world);
    // The instances that the loaded scene gives, in the same order: its nodes', then their meshes'.
    let frame = drawing
        .builder
        .build(&world, &mut drawing.cache, &gpu, 0.0)
        .expect("the frame is built");
    assert_eq!(frame.instances.len(), transforms.len());
    for (instance, expected) in frame.instances.iter().zip(&transforms) {
        assert!(
            instance.transform.abs_diff_eq(*expected, 1e-5),
            "{} against {expected}",
            instance.transform
        );
    }
    drop(frame);
    // And the glTF test's images.
    for (name, yaw, pitch) in [("gltf-front", 0.6, 0.45), ("gltf-back", 3.6, 0.25)] {
        let (camera, distance) = camera(&bounds, yaw, pitch);
        reach(&mut world, distance);
        let (image, stats) = drawing.draw(&gpu, &target, &world, camera);
        assert_eq!((stats.draw_calls, stats.triangles), (7, 2274));
        if !reference_image::update_requested() {
            reference_image::check(name, &format!("world-{name}"), &image, gpu.adapter_name());
        }
    }
}

#[test]
fn a_frame_built_on_one_thread_is_drawn_on_another_as_on_the_first() {
    let gpu = gpu();
    let (bounds, _) = loaded(&gpu);
    let (camera, distance) = camera(&bounds, 0.6, 0.45);
    let mut world = world();
    reach(&mut world, distance);
    let mut drawing = Drawing::new(&gpu);
    let (here, _) = drawing.draw(&gpu, &target(&gpu), &world, camera);
    let frame = drawing
        .builder
        .build(&world, &mut drawing.cache, &gpu, 0.0)
        .expect("the frame is built");
    // The frame owns what it draws: the cache and the builder go, and the renderer goes to the
    // other thread.
    let Drawing {
        mut renderer,
        cache,
        builder,
    } = drawing;
    drop((cache, builder));
    let there = std::thread::spawn({
        let gpu = gpu.clone();
        move || {
            let target = target(&gpu);
            let mut scene = frame.scene(camera);
            scene.exposure = EXPOSURE;
            scene.bloom = BLOOM;
            renderer
                .render(&gpu, &target, &scene)
                .expect("the frame is drawn");
            gpu.read_render_target(&target).expect("read back")
        }
    })
    .join()
    .expect("the other thread draws");
    assert!(here.pixels == there.pixels, "the same image");
}

/// Where the frame at `fraction` draws the sample's ring, and its light.
fn ring_at(drawing: &mut Drawing, gpu: &Gpu, world: &World, fraction: f64) -> (f64, f64) {
    let ring = entity(world, "ring");
    let mesh = world
        .get::<Meshes>(ring)
        .expect("the ring places a mesh")
        .ids[0];
    // The cache holds everything already: building the frame takes nothing in, and the mesh stays
    // where it is.
    let ring_mesh = drawing.cache.mesh(mesh).expect("held").0.clone();
    let frame = drawing
        .builder
        .build(world, &mut drawing.cache, gpu, fraction)
        .expect("the frame is built");
    let instance = frame
        .instances
        .iter()
        .find(|instance| instance.mesh == ring_mesh)
        .expect("the ring is drawn");
    let light = frame
        .lights
        .iter()
        .find_map(|light| match light {
            Light::Point(light) => Some(light.position),
            _ => None,
        })
        .expect("the ring's light is drawn");
    (instance.transform.translation.x, light.x)
}

/// Moves the sample's ring by `x` along x during a fixed step.
fn step(world: &mut World, x: f64) {
    let ring = entity(world, "ring");
    world.begin_fixed_step();
    world
        .get_mut::<Transform>(ring)
        .expect("placed")
        .translation
        .x += x;
    world.end_fixed_step();
}

#[test]
fn what_moves_at_fixed_steps_is_drawn_where_the_fraction_of_a_step_puts_it() {
    let gpu = gpu();
    let mut world = world();
    let mut drawing = Drawing::new(&gpu);
    drawing.settle(&gpu, &world);
    let (ring, light) = ring_at(&mut drawing, &gpu, &world, 0.0);
    step(&mut world, 1.0);
    for fraction in [0.0, 0.25, 1.0] {
        let (drawn, lit) = ring_at(&mut drawing, &gpu, &world, fraction);
        assert!(
            (drawn - ring - fraction).abs() < 1e-6,
            "{fraction}: {drawn}"
        );
        assert!((lit - light - fraction).abs() < 1e-6, "{fraction}: {lit}");
    }
    // A frame without a step keeps the last step's two places.
    let (drawn, _) = ring_at(&mut drawing, &gpu, &world, 0.5);
    assert!((drawn - ring - 0.5).abs() < 1e-6, "{drawn}");
    // A frame with two steps takes the last one's.
    step(&mut world, 1.0);
    step(&mut world, 1.0);
    let (drawn, _) = ring_at(&mut drawing, &gpu, &world, 0.5);
    assert!((drawn - ring - 2.5).abs() < 1e-6, "{drawn}");
    // A teleport does not slide.
    let teleported = entity(&world, "ring");
    world.begin_fixed_step();
    world
        .get_mut::<Transform>(teleported)
        .expect("placed")
        .translation
        .x += 10.0;
    world.teleport(teleported);
    world.end_fixed_step();
    let (drawn, _) = ring_at(&mut drawing, &gpu, &world, 0.0);
    assert!((drawn - ring - 13.0).abs() < 1e-6, "{drawn}");
}

#[test]
fn a_hidden_subtree_and_its_lights_are_not_drawn() {
    let gpu = gpu();
    let mut world = world();
    let mut drawing = Drawing::new(&gpu);
    drawing.settle(&gpu, &world);
    let counted = |drawing: &mut Drawing, world: &World| {
        let frame = drawing
            .builder
            .build(world, &mut drawing.cache, &gpu, 0.0)
            .expect("the frame is built");
        let points = frame
            .lights
            .iter()
            .filter(|light| matches!(light, Light::Point(_)))
            .count();
        (frame.instances.len(), points)
    };
    let (instances, points) = counted(&mut drawing, &world);
    // The ring, and the light it holds.
    world.insert(entity(&world, "ring"), Hidden);
    assert_eq!(counted(&mut drawing, &world), (instances - 1, points - 1));
    // The props, an empty node that holds the sphere and the pillar.
    world.insert(entity(&world, "props"), Hidden);
    assert_eq!(counted(&mut drawing, &world), (instances - 3, points - 1));
    world.remove::<Hidden>(entity(&world, "ring"));
    world.remove::<Hidden>(entity(&world, "props"));
    assert_eq!(counted(&mut drawing, &world), (instances, points));
}

#[test]
fn after_a_lost_device_the_cache_made_again_draws_the_same_world() {
    let gpu = gpu();
    let (bounds, _) = loaded(&gpu);
    let mut world = world();
    let (camera, distance) = camera(&bounds, 0.6, 0.45);
    reach(&mut world, distance);
    let mut drawing = Drawing::new(&gpu);
    let (before, _) = drawing.draw(&gpu, &target(&gpu), &world, camera);
    let ring = *world
        .get::<Transform>(entity(&world, "ring"))
        .expect("placed");

    // What a game does when its device is lost: its GPU objects go, its world stays.
    gpu.lose();
    drawing.builder.release();
    let Drawing {
        renderer,
        cache,
        builder,
    } = drawing;
    // Nothing may hold the lost device when the new one opens (`Gpu::restore`): the renderer goes,
    // with the cache that loads through it.
    drop((renderer, cache));
    let gpu = gpu.restore().expect("a new device opens");
    let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let mut drawing = Drawing {
        cache: AssetCache::new(&renderer, sample().clone()),
        renderer,
        builder,
    };
    let (after, _) = drawing.draw(&gpu, &target(&gpu), &world, camera);
    assert_eq!(
        world.get::<Transform>(entity(&world, "ring")),
        Some(&ring),
        "the world is as it was"
    );
    assert!(
        before.pixels == after.pixels,
        "the same image after the loss"
    );
}

/// The examples' yard spawned from `scenes` under the key `yard`, as the `world` example spawns it.
fn yard(scenes: &mut SceneLibrary) -> World {
    let mut world = World::new();
    let yard = scenes
        .resolve(world.registry(), YARD)
        .expect("the yard resolves");
    world.spawn_scene("yard", &yard).expect("spawned");
    world
}

/// The color of the light of the yard's lamp `lamp`, if it has a light.
fn lamp_color(world: &World, lamp: &str) -> Option<[f32; 3]> {
    let light = world.find(&PersistentId::new(["yard", lamp, "light"]))?;
    match world.get::<slime_world::Light>(light)? {
        slime_world::Light::Point { color, .. } => Some([color.r, color.g, color.b]),
        _ => None,
    }
}

#[test]
fn the_cooked_yard_spawns_the_sample_and_four_lamps_with_their_overrides() {
    let mut scenes = SceneLibrary::of(sample().clone());
    let world = yard(&mut scenes);
    assert!(
        world
            .find(&PersistentId::new(["yard", "sample", "ring"]))
            .is_some()
    );
    let warm = Some([1.0, 0.75, 0.45]);
    assert_eq!(lamp_color(&world, "lamp 1"), warm);
    assert_eq!(lamp_color(&world, "lamp 2"), Some([1.0, 0.1, 0.05]), "red");
    assert_eq!(lamp_color(&world, "lamp 3"), warm);
    assert_eq!(lamp_color(&world, "lamp 4"), None, "without its light");
    let lamp = world
        .find(&PersistentId::new(["yard", "lamp 2"]))
        .expect("the second lamp");
    let at = world.get::<Transform>(lamp).expect("placed").translation;
    assert_eq!((at.x, at.z), (-3.5, 3.5));
    assert!(world.placement(lamp).is_some());
}

#[test]
fn a_lamp_cooked_again_reaches_every_lamp_but_where_the_yard_overrides_it() {
    // The examples' sources, copied, so that the test changes the lamp.
    let root = cooked::directory("lamp-changed");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("the directory can be made");
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    for name in ["scene.glb", "lamp.scene", "yard.scene"] {
        for file in [name.to_owned(), format!("{name}.meta")] {
            fs::copy(assets.join(&file), sources.join(&file)).expect("the source is copied");
        }
    }
    let output = root.join("cooked");
    slime_cook::cook(&sources, &output).expect("cooked");
    let open = || slime_assets::CookedAssets::open(&output).expect("the cooked data");
    let mut scenes = SceneLibrary::of(open());
    let old_start = yard(&mut scenes);
    let mut run = yard(&mut scenes);
    let ring = run
        .find(&PersistentId::new(["yard", "sample", "ring"]))
        .expect("the ring");
    run.get_mut::<Transform>(ring)
        .expect("placed")
        .translation
        .y = 1.25;
    // The lamp's light turns green, and its glass grows.
    let lamp = fs::read_to_string(sources.join("lamp.scene")).expect("the lamp is read");
    let changed = lamp
        .replace("g = 0.75, b = 0.45", "g = 1.0, b = 0.2")
        .replace("scale = 0.4", "scale = 0.5");
    assert_ne!(changed, lamp);
    fs::write(sources.join("lamp.scene"), changed).expect("the lamp is written");
    slime_cook::cook(&sources, &output).expect("cooked again");
    assert_eq!(scenes.reload(open()).len(), 1, "the lamp changed");
    let (carried, loaded) = run
        .carry(&old_start, yard(&mut scenes), &mut scenes)
        .expect("carried");
    assert!(loaded.set_aside.is_empty(), "{:?}", loaded.set_aside);
    let green = Some([1.0, 1.0, 0.2]);
    assert_eq!(lamp_color(&carried, "lamp 1"), green);
    assert_eq!(
        lamp_color(&carried, "lamp 2"),
        Some([1.0, 0.1, 0.05]),
        "its override stays"
    );
    assert_eq!(lamp_color(&carried, "lamp 3"), green);
    assert_eq!(lamp_color(&carried, "lamp 4"), None);
    for lamp in ["lamp 1", "lamp 2", "lamp 3", "lamp 4"] {
        let glass = carried
            .find(&PersistentId::new(["yard", lamp, "glass"]))
            .expect("a glass");
        assert_eq!(
            carried.get::<Transform>(glass).expect("placed").scale,
            0.5,
            "{lamp}"
        );
    }
    let ring = carried
        .find(&PersistentId::new(["yard", "sample", "ring"]))
        .expect("the ring");
    assert_eq!(
        carried
            .get::<Transform>(ring)
            .expect("placed")
            .translation
            .y,
        1.25,
        "what the run changed stays"
    );
}
