//! The world's benchmarks: a frame built from many entities, their transforms, a large hierarchy
//! changed, a scene file of many placements resolved, and frames built around a sun that does not
//! move.

use std::thread;
use std::time::{Duration, Instant};

use slime_assets::AssetId;
use slime_color::LinearColor;
use slime_render::glam::{DVec3, Quat};
use slime_render::{AssetCache, Renderer};
use slime_world::{Entity, Light, Meshes, SceneLibrary, Transform, World};
use slime_world_render::FrameBuilder;

use crate::measure::{Context, Row, meshes_of, ms, percentile, timed};
use crate::render::{FORMAT, sky_and_sun};

/// The parents of the large world, and the children of each.
const PARENTS: usize = 1_000;
const CHILDREN: usize = 100;
/// How long the meshes of a frame may take to load before a benchmark gives up.
const LOAD_LIMIT: Duration = Duration::from_secs(120);

/// A world of `PARENTS` entities on a grid, each the parent of `CHILDREN` that place one of
/// `meshes` each, in turn, on a grid of their own: its parents, and its transforms computed.
fn large_world(meshes: &[AssetId]) -> Result<(World, Vec<Entity>), String> {
    let mut world = World::new();
    let mut parents = Vec::with_capacity(PARENTS);
    for index in 0..PARENTS {
        let parent = world.spawn();
        let (x, z) = ((index % 32) as f64, (index / 32) as f64);
        world.insert(
            parent,
            Transform::from_translation(DVec3::new(x * 40.0, 0.0, z * 40.0)),
        );
        for child_index in 0..CHILDREN {
            let child = world.spawn();
            let (x, z) = ((child_index % 10) as f64, (child_index / 10) as f64);
            world.insert(
                child,
                Transform::from_translation(DVec3::new(x * 3.0, 0.0, z * 3.0)),
            );
            if !meshes.is_empty() {
                world.insert(
                    child,
                    Meshes {
                        ids: vec![meshes[child_index % meshes.len()]],
                        casts_shadows: true,
                    },
                );
            }
            world
                .set_parent(child, Some(parent))
                .map_err(|error| error.to_string())?;
        }
        parents.push(parent);
    }
    world.update_transforms();
    Ok((world, parents))
}

/// Builds frames of `world` until the cache holds every mesh it places, and returns how many
/// instances the last one draws.
fn load_all(
    world: &World,
    builder: &mut FrameBuilder,
    cache: &mut AssetCache,
    renderer: &Renderer,
    context: &Context,
) -> Result<usize, String> {
    let start = Instant::now();
    loop {
        let frame = builder
            .build(world, cache, renderer, &context.gpu, 0.0)
            .map_err(|error| error.to_string())?;
        if let Some(error) = frame.errors.first() {
            return Err(error.to_string());
        }
        if frame.loading == 0 {
            return Ok(frame.instances.len());
        }
        if start.elapsed() > LOAD_LIMIT {
            return Err(format!("the meshes did not load in {LOAD_LIMIT:?}"));
        }
        drop(frame);
        thread::sleep(Duration::from_millis(1));
    }
}

/// `FrameBuilder::build` over the large world, its meshes those of the examples' sample, once the
/// cache holds them all: twenty frames, without a sky.
pub fn frame(context: &mut Context) -> Result<Vec<Row>, String> {
    let assets = context.sample()?;
    let (world, _) = large_world(&meshes_of(&assets))?;
    let renderer = Renderer::new(&context.gpu, FORMAT).map_err(|error| error.to_string())?;
    let mut cache = AssetCache::new(assets);
    let mut builder = FrameBuilder::new(None);
    let drawn = load_all(&world, &mut builder, &mut cache, &renderer, context)?;
    let mut times = Vec::new();
    for _ in 0..20 {
        let (frame, duration) =
            timed(|| builder.build(&world, &mut cache, &renderer, &context.gpu, 0.0));
        frame.map_err(|error| error.to_string())?;
        times.push(duration);
    }
    let mut rows = Vec::from(Row::times("`FrameBuilder::build`", &times));
    rows.push(Row::new("instances in a frame", drawn.to_string()));
    Ok(rows)
}

/// `World::update_transforms` over the large world, once all its parents moved: twenty times.
pub fn transforms(_context: &mut Context) -> Result<Vec<Row>, String> {
    let (mut world, parents) = large_world(&[])?;
    let mut times = Vec::new();
    for step in 0..20 {
        for (index, &parent) in parents.iter().enumerate() {
            let (x, z) = ((index % 32) as f64, (index / 32) as f64);
            world.insert(
                parent,
                Transform::from_translation(DVec3::new(x * 40.0, f64::from(step), z * 40.0)),
            );
        }
        let ((), duration) = timed(|| world.update_transforms());
        times.push(duration);
    }
    Ok(Vec::from(Row::times(
        "`World::update_transforms` after 1 000 parents moved",
        &times,
    )))
}

/// A parent of 10 000, then 20 000 children, each of which is given no parent in turn: once each.
pub fn children(_context: &mut Context) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    for count in [10_000, 20_000] {
        let mut world = World::new();
        let parent = world.spawn();
        let mut children = Vec::with_capacity(count);
        for _ in 0..count {
            let child = world.spawn();
            world
                .set_parent(child, Some(parent))
                .map_err(|error| error.to_string())?;
            children.push(child);
        }
        let (emptied, duration) = timed(|| {
            children
                .iter()
                .try_for_each(|&child| world.set_parent(child, None))
        });
        emptied.map_err(|error| error.to_string())?;
        rows.push(Row::new(
            format!("a parent of {count} children emptied"),
            ms(duration),
        ));
    }
    Ok(rows)
}

/// A scene file that places a lamp, a prefab of a post and its light, `count` times, each lamp's
/// light changed: 2 000, then 8 000 placements, resolved three times each.
pub fn placements(_context: &mut Context) -> Result<Vec<Row>, String> {
    let lamp = "[entities.post.components]\nName = \"post\"\n\
                Transform = { translation = { x = 0.0, y = 1.0, z = 0.0 } }\n\n\
                [entities.light]\nparent = \"post\"\n\n[entities.light.components]\n\
                Light = { Point = { color = { r = 1.0, g = 0.8, b = 0.6, a = 1.0 }, \
                intensity = 40.0 } }\n";
    let (lamp_id, yard_id) = (AssetId::from_u128(1), AssetId::from_u128(2));
    let registry = World::new();
    let mut rows = Vec::new();
    for count in [2_000, 8_000] {
        let mut yard = format!("[assets]\nlamp = {{ id = \"{lamp_id}\" }}\n\n");
        for index in 0..count {
            yard.push_str(&format!("[entities.lamp{index}]\nprefab = \"lamp\"\n\n"));
        }
        for index in 0..count {
            yard.push_str(&format!(
                "[changed.\"lamp{index}/light\".Light.Point]\nintensity = 10.0\n\n"
            ));
        }
        let mut times = Vec::new();
        for _ in 0..3 {
            let mut scenes = SceneLibrary::new();
            scenes.insert_file(lamp_id, "lamp", lamp);
            scenes.insert_file(yard_id, "yard", yard.clone());
            let (resolved, duration) = timed(|| scenes.resolve(registry.registry(), yard_id));
            resolved.map_err(|error| error.to_string())?;
            times.push(duration);
        }
        rows.push(Row::new(
            format!("{count} placements resolved, median of 3"),
            ms(percentile(&times, 0.5)),
        ));
    }
    Ok(rows)
}

/// Frames built around a sun that does not move, after fixed steps that set the sun where it
/// already is, as a game that sets the time of day at each step does: twenty-four frames, with the
/// examples' sky, at the fractions of a fixed step at which a display of 144 Hz shows frames of a
/// game that steps at 60 Hz.
pub fn sky(context: &mut Context) -> Result<Vec<Row>, String> {
    let assets = context.sample()?;
    let (sky, _) = sky_and_sun();
    let mut world = World::new();
    let sun = world.spawn();
    let at = Transform {
        rotation: Quat::from_rotation_x(-0.9) * Quat::from_rotation_y(0.4),
        ..Transform::IDENTITY
    };
    world.insert(sun, at);
    world.insert(
        sun,
        Light::Directional {
            color: LinearColor::rgb(1.0, 0.98, 0.9),
            illuminance: 100_000.0,
            casts_shadows: true,
            shadow_distance: None,
            in_sky: true,
        },
    );
    world.update_transforms();
    let renderer = Renderer::new(&context.gpu, FORMAT).map_err(|error| error.to_string())?;
    let mut cache = AssetCache::new(assets);
    let mut builder = FrameBuilder::new(Some(slime_render::Sky { sun: None, ..sky }));
    let mut times = Vec::new();
    for frame in 0..24 {
        world.begin_fixed_step();
        if let Some(transform) = world.get_mut::<Transform>(sun) {
            *transform = at;
        }
        world.end_fixed_step();
        world.update_transforms();
        let fraction = (f64::from(frame) * 60.0 / 144.0).fract();
        let (built, duration) =
            timed(|| builder.build(&world, &mut cache, &renderer, &context.gpu, fraction));
        built.map_err(|error| error.to_string())?;
        times.push(duration);
    }
    // A frame that makes the sky's environment again takes tens of milliseconds, one that keeps it
    // a few microseconds.
    let remade = times
        .iter()
        .filter(|&&time| time > Duration::from_millis(1))
        .count();
    Ok(vec![
        Row::new(
            "`FrameBuilder::build`, median of 24",
            ms(percentile(&times, 0.5)),
        ),
        Row::new(
            "`FrameBuilder::build`, slowest of 24",
            ms(percentile(&times, 1.0)),
        ),
        Row::new(
            "frames that made the sky again, longer than 1 ms, of 24",
            remade.to_string(),
        ),
    ])
}
