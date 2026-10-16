//! The place the game is played in: its ground, the colliders that stop the hero, where the hero
//! starts, and the places of the game's loop: the key, the door it opens, the campfire. Positions
//! are x and z in metres, on the ground, at y = 0, +y up; north is −z and east +x; an angle about
//! the vertical is in radians from +x towards +z.
//!
//! The zone comes from the cooked data: `games/hunnawood/zone/build_zone.py` builds it in Blender,
//! the cooker cooks it, and [`Zone::from_scene`] reads it from the custom values (`extras`) of the
//! cooked scene's nodes, as `README.md` describes ("The zone").

use std::collections::HashSet;

use slime_assets::{CookedScene, Extra, SceneNode};
use slime_render::glam::{Mat4, Vec2};

/// The footprint of a collider on the ground, around its center.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A rectangle of half the size `half_size` along its own axes, turned by `yaw` about the
    /// vertical: its first axis lies along `Vec2::from_angle(yaw)`.
    Box { half_size: Vec2, yaw: f32 },
    /// A circle.
    Cylinder { radius: f32 },
}

/// What stops the hero: a box turned about the vertical or a cylinder, standing on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Collider {
    pub shape: Shape,
    /// The center of its footprint.
    pub center: Vec2,
    /// How high it stands, in metres.
    pub height: f32,
}

impl Collider {
    /// Where a circle of `radius` around `position` must move to overlap the footprint no more,
    /// if it overlaps it: away from the footprint's nearest point, which keeps the part of a move
    /// along the footprint's side, so that what moves slides along it. A center inside a box leaves
    /// through the nearest side.
    pub fn push_out(&self, position: Vec2, radius: f32) -> Option<Vec2> {
        match self.shape {
            Shape::Box { half_size, yaw } => {
                // In the box's own axes, where its sides lie along x and y.
                let local = Vec2::from_angle(-yaw).rotate(position - self.center);
                let away = local - local.clamp(-half_size, half_size);
                let distance = away.length();
                if distance >= radius {
                    return None;
                }
                let out = if distance > 1e-6 {
                    local + away * ((radius - distance) / distance)
                } else {
                    let depth = half_size - local.abs();
                    if depth.x < depth.y {
                        Vec2::new(local.x.signum() * (half_size.x + radius), local.y)
                    } else {
                        Vec2::new(local.x, local.y.signum() * (half_size.y + radius))
                    }
                };
                Some(self.center + Vec2::from_angle(yaw).rotate(out))
            }
            Shape::Cylinder { radius: own_radius } => {
                let offset = position - self.center;
                let distance = offset.length();
                let reach = own_radius + radius;
                if distance >= reach {
                    None
                } else if distance > 1e-6 {
                    Some(self.center + offset * (reach / distance))
                } else {
                    Some(self.center + Vec2::X * reach)
                }
            }
        }
    }

    /// Whether `point`, on the ground, lies within `margin` of the footprint, or inside it.
    pub fn covers(&self, point: Vec2, margin: f32) -> bool {
        match self.shape {
            Shape::Box { half_size, yaw } => {
                let local = Vec2::from_angle(-yaw).rotate(point - self.center);
                local.abs().cmple(half_size + margin).all()
            }
            Shape::Cylinder { radius } => point.distance(self.center) <= radius + margin,
        }
    }
}

/// A zone: its ground, its colliders, where the hero starts, and the places of the game's loop.
#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    /// Half the size of the ground along x and z, around the origin; the hero does not leave it.
    pub half_size: Vec2,
    pub colliders: Vec<Collider>,
    /// Where the hero starts.
    pub start: Vec2,
    /// Which way the hero and the camera look at the start, of length 1.
    pub facing: Vec2,
    /// Where the key lies, which opens the door.
    pub item: Vec2,
    /// The middle of the sealed door.
    pub door: Vec2,
    /// The campfire, where the hero rests.
    pub campfire: Vec2,
    /// The terrace behind the door.
    pub terrace: Vec2,
    /// The indices in `colliders` of those that close the door, which open with it.
    pub door_colliders: Vec<usize>,
    /// The scene's nodes that are the key, which the game stops drawing once the hero has it.
    pub item_nodes: HashSet<usize>,
    /// The scene's nodes that are the door's leaves, which the game stops drawing once it opens.
    pub door_nodes: HashSet<usize>,
}

impl Zone {
    /// Whether `other` lays the game out as this zone does: the same ground, colliders, start and
    /// places. The nodes that are the key and the door's leaves may differ, as when a scene's nodes
    /// are cooked in another order: they say what is drawn, not where the game is played.
    pub fn same_layout(&self, other: &Zone) -> bool {
        Zone {
            item_nodes: other.item_nodes.clone(),
            door_nodes: other.door_nodes.clone(),
            ..self.clone()
        } == *other
    }
}

/// The text of `node`'s custom value `name`, if it has one that is a text.
pub fn text<'a>(node: &'a SceneNode, name: &str) -> Option<&'a str> {
    node.extras
        .iter()
        .find(|(n, _)| n == name)
        .and_then(|(_, extra)| match extra {
            Extra::Text(text) => Some(text.as_str()),
            _ => None,
        })
}

/// The number of `node`'s custom value `name`, if it has one that is a number.
fn number(node: &SceneNode, name: &str) -> Option<f32> {
    node.extras
        .iter()
        .find(|(n, _)| n == name)
        .and_then(|(_, extra)| match extra {
            Extra::Number(number) => Some(*number as f32),
            _ => None,
        })
}

/// The indices of `ancestor` and of every node below it.
fn subtree(scene: &CookedScene, ancestor: usize) -> HashSet<usize> {
    // A node comes after its parent, so one pass in order finds every descendant.
    let mut nodes = HashSet::from([ancestor]);
    for (index, node) in scene.nodes.iter().enumerate() {
        if node
            .parent
            .is_some_and(|parent| nodes.contains(&(parent as usize)))
        {
            nodes.insert(index);
        }
    }
    nodes
}

impl Zone {
    /// The zone that the cooked `scene` holds: a collider for each node whose `collider` is
    /// `"box"` (its scale is half its size, turned about the vertical only) or `"cylinder"` (its
    /// scale is its radius), `height` metres high; and one node for each `marker`: `start`, facing
    /// along its −z axis; `item`, `door` and `campfire`; `terrace`; `bounds`, whose scale is half
    /// the size of the ground.
    ///
    /// # Errors
    ///
    /// A message that says what is wrong: a marker missing or given twice, a collider of another
    /// kind or without a height.
    pub fn from_scene(scene: &CookedScene) -> Result<Self, String> {
        let world: Vec<Mat4> = scene
            .world_transforms()
            .iter()
            .map(Mat4::from_cols_array)
            .collect();
        let ground = |m: &Mat4| Vec2::new(m.w_axis.x, m.w_axis.z);
        let marker = |kind: &str| {
            let found: Vec<usize> = (0..scene.nodes.len())
                .filter(|&i| text(&scene.nodes[i], "marker") == Some(kind))
                .collect();
            match found[..] {
                [index] => Ok(index),
                _ => Err(format!(
                    "scene `{}`: {} markers `{kind}`, where one is needed",
                    scene.name,
                    found.len()
                )),
            }
        };
        let item = marker("item")?;
        let door = marker("door")?;
        let door_nodes = subtree(scene, door);
        let mut colliders = Vec::new();
        let mut door_colliders = Vec::new();
        for (index, node) in scene.nodes.iter().enumerate() {
            let Some(kind) = text(node, "collider") else {
                continue;
            };
            let m = world[index];
            let height = number(node, "height")
                .ok_or_else(|| format!("collider `{}` has no height", node.name))?;
            let shape = match kind {
                "box" => Shape::Box {
                    half_size: Vec2::new(
                        m.x_axis.truncate().length(),
                        m.z_axis.truncate().length(),
                    ),
                    yaw: m.x_axis.z.atan2(m.x_axis.x),
                },
                "cylinder" => Shape::Cylinder {
                    radius: m.x_axis.truncate().length(),
                },
                other => return Err(format!("collider `{}` is a `{other}`", node.name)),
            };
            if door_nodes.contains(&index) {
                door_colliders.push(colliders.len());
            }
            colliders.push(Collider {
                shape,
                center: ground(&m),
                height,
            });
        }
        let start = world[marker("start")?];
        let bounds = world[marker("bounds")?];
        Ok(Self {
            half_size: Vec2::new(
                bounds.x_axis.truncate().length(),
                bounds.z_axis.truncate().length(),
            ),
            colliders,
            start: ground(&start),
            facing: Vec2::new(-start.z_axis.x, -start.z_axis.z).normalize(),
            item: ground(&world[item]),
            door: ground(&world[door]),
            campfire: ground(&world[marker("campfire")?]),
            terrace: ground(&world[marker("terrace")?]),
            door_colliders,
            item_nodes: subtree(scene, item),
            door_nodes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turned_box_covers_its_turned_footprint() {
        // 4 m long along its first axis, turned a quarter turn: it lies along z.
        let collider = Collider {
            shape: Shape::Box {
                half_size: Vec2::new(2.0, 0.5),
                yaw: std::f32::consts::FRAC_PI_2,
            },
            center: Vec2::ZERO,
            height: 1.0,
        };
        assert!(collider.covers(Vec2::new(0.0, 1.9), 0.0));
        assert!(!collider.covers(Vec2::new(1.9, 0.0), 0.0));
        assert!(collider.covers(Vec2::new(0.7, 0.0), 0.25));
        assert!(!collider.covers(Vec2::new(0.8, 0.0), 0.25));
    }
}

/// The tests that read the cooked zone, which they need fetched and cooked
/// (`scripts/check.sh` does both): `cargo test` leaves them out.
#[cfg(test)]
pub mod cooked {
    use std::collections::{HashSet, VecDeque};

    use slime_assets::{AssetKind, CookedAssets, LightKind};
    use slime_render::glam::Vec2;

    use super::{Collider, Zone, text};
    use crate::assets;
    use crate::rules::HERO_RADIUS;

    /// The cooked zone, from `target/cooked/hunnawood`.
    pub fn zone() -> (CookedAssets, Zone) {
        let assets = CookedAssets::open(assets::cooked_in_target())
            .unwrap_or_else(|error| panic!("{error}; {}", assets::HOW_TO_COOK));
        let scene = assets.scene(assets::zone_id()).expect("the zone is cooked");
        let zone = Zone::from_scene(&scene).expect("the zone is whole");
        (assets, zone)
    }

    /// The cells of a grid of `CELL` metres over the ground that the hero reaches from the start,
    /// walking between `colliders`.
    struct Reach {
        origin: Vec2,
        columns: usize,
        reached: Vec<bool>,
    }

    const CELL: f32 = 0.25;

    impl Reach {
        fn new(zone: &Zone, colliders: &[Collider]) -> Self {
            let origin = -zone.half_size;
            let columns = (2.0 * zone.half_size.x / CELL) as usize;
            let rows = (2.0 * zone.half_size.y / CELL) as usize;
            let center =
                |c: usize, r: usize| origin + Vec2::new(c as f32 + 0.5, r as f32 + 0.5) * CELL;
            let free = |p: Vec2| {
                p.abs().cmplt(zone.half_size - HERO_RADIUS).all()
                    && !colliders.iter().any(|c| c.covers(p, HERO_RADIUS))
            };
            let mut reached = vec![false; columns * rows];
            let start = ((zone.start - origin) / CELL).as_uvec2();
            let (sc, sr) = (start.x as usize, start.y as usize);
            assert!(free(center(sc, sr)), "the start is not free");
            let mut queue = VecDeque::from([(sc, sr)]);
            reached[sr * columns + sc] = true;
            while let Some((c, r)) = queue.pop_front() {
                let neighbours = [
                    (c.wrapping_sub(1), r),
                    (c + 1, r),
                    (c, r.wrapping_sub(1)),
                    (c, r + 1),
                ];
                for (nc, nr) in neighbours {
                    if nc >= columns || nr >= rows || reached[nr * columns + nc] {
                        continue;
                    }
                    if free(center(nc, nr)) {
                        reached[nr * columns + nc] = true;
                        queue.push_back((nc, nr));
                    }
                }
            }
            Self {
                origin,
                columns,
                reached,
            }
        }

        /// Whether the hero comes within `distance` of `point`.
        fn near(&self, point: Vec2, distance: f32) -> bool {
            self.reached.iter().enumerate().any(|(i, &reached)| {
                let cell = self.origin
                    + Vec2::new(
                        (i % self.columns) as f32 + 0.5,
                        (i / self.columns) as f32 + 0.5,
                    ) * CELL;
                reached && cell.distance(point) <= distance
            })
        }
    }

    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_cooked_zone_has_its_places_and_its_start_is_clear() {
        let (_, zone) = zone();
        assert!(zone.start.abs().cmplt(zone.half_size - 1.0).all());
        for collider in &zone.colliders {
            assert!(!collider.covers(zone.start, HERO_RADIUS), "{collider:?}");
        }
        assert!((zone.facing.length() - 1.0).abs() < 1e-4);
        assert!(!zone.door_colliders.is_empty(), "the door has a collider");
        assert!(zone.item_nodes.len() > 1, "the key has a mesh");
        assert!(zone.door_nodes.len() > 1, "the door has leaves");
        for place in [zone.item, zone.door, zone.campfire, zone.terrace] {
            assert!(place.abs().cmplt(zone.half_size).all(), "{place}");
        }
    }

    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_hero_reaches_the_item_and_the_campfire_and_the_door_keeps_the_terrace() {
        let (_, zone) = zone();
        let closed = Reach::new(&zone, &zone.colliders);
        assert!(closed.near(zone.item, 1.2), "the item is out of reach");
        assert!(
            closed.near(zone.campfire, 2.0),
            "the campfire is out of reach"
        );
        assert!(closed.near(zone.door, 1.5), "the door is out of reach");
        assert!(
            !closed.near(zone.terrace, 2.0),
            "the terrace is reached without the door"
        );
        // Once the door opens, the terrace is reached through it.
        let open: Vec<Collider> = (0..zone.colliders.len())
            .filter(|i| !zone.door_colliders.contains(i))
            .map(|i| zone.colliders[i])
            .collect();
        assert!(
            Reach::new(&zone, &open).near(zone.terrace, 1.0),
            "the open door leads nowhere"
        );
    }

    /// The zone's budgets: what the renderer draws in every frame (it culls nothing, so every frame
    /// draws all of it), the lights that cast shadows at night, the GPU memory of its textures and
    /// the size of its cooked data. They were set at Stage 4a, before the zone was made, from a
    /// synthetic forest measured on an RX 6900 XT and on lavapipe.
    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_cooked_zone_keeps_within_its_budgets() {
        let (assets, _) = zone();
        let scene = assets.scene(assets::zone_id()).expect("the zone is cooked");
        let mut triangles = 0;
        let mut instances = 0;
        let mut meshes = std::collections::HashMap::new();
        for node in &scene.nodes {
            for &id in &node.meshes {
                let count = *meshes.entry(id).or_insert_with(|| {
                    assets.mesh(id).expect("the mesh is cooked").indices.len() / 3
                });
                triangles += count;
                instances += 1;
            }
        }
        let night_spots = scene
            .nodes
            .iter()
            .filter(|n| matches!(n.light.map(|l| l.kind), Some(LightKind::Spot { .. })))
            .filter(|n| text(n, "time") == Some("night"))
            .count();
        // Every asset the scene needs, through its meshes and materials, each once.
        let manifest = assets.manifest();
        let mut seen = HashSet::new();
        let mut stack = vec![assets::zone_id()];
        let (mut size, mut texture_bytes) = (0u64, 0u64);
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            let entry = manifest.get(id).expect("listed");
            size += entry.size;
            if entry.kind == AssetKind::Texture {
                let texture = assets.texture(id).expect("the texture is cooked");
                texture_bytes += texture.levels.iter().map(|l| l.len() as u64).sum::<u64>();
            }
            stack.extend(entry.needs.iter().copied());
        }
        let mib = |bytes: u64| bytes as f64 / (1 << 20) as f64;
        eprintln!(
            "zone: {triangles} triangles, {instances} instances, {night_spots} spot lights at \
             night, textures {:.0} MiB, cooked {:.0} MiB",
            mib(texture_bytes),
            mib(size)
        );
        assert!(triangles <= 1_000_000, "{triangles} triangles");
        assert!(instances <= 800, "{instances} instances");
        assert!(night_spots <= 6, "{night_spots} spot lights");
        assert!(
            mib(texture_bytes) <= 384.0,
            "{:.0} MiB of textures",
            mib(texture_bytes)
        );
        assert!(mib(size) <= 320.0, "{:.0} MiB of cooked data", mib(size));
    }

    /// The zone spawned in a world, each node an entity placed where the file puts it, under its
    /// name: the world of Stage 5a holds what the game reads from the scene today.
    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_cooked_zone_spawns_in_a_world_as_its_file_places_it() {
        use slime_render::glam::{DMat4, Mat4};
        use slime_world::{PersistentId, Transform, World};

        let (assets, _) = zone();
        let scene = assets.scene(assets::zone_id()).expect("the zone is cooked");
        let mut world = World::new();
        world.spawn_scene("zone", &scene).expect("spawned");
        world.update_transforms();
        assert_eq!(
            world.len(),
            scene.nodes.len() + 1,
            "a root and an entity per node"
        );
        for (node, expected) in scene.nodes.iter().zip(scene.world_transforms()) {
            // Every node of the zone has a name of its own, so none is known by its index.
            let entity = world
                .find(&PersistentId::new(["zone", node.name.as_str()]))
                .unwrap_or_else(|| panic!("node `{}` is known by its name", node.name));
            let stretch = world.get::<Transform>(entity).expect("placed").stretch;
            let placed = world
                .global_transform(entity)
                .expect("placed")
                .to_affine(stretch);
            let cells = DMat4::from(placed).to_cols_array();
            let expected = Mat4::from_cols_array(&expected).to_cols_array();
            for (a, b) in cells.iter().zip(expected) {
                assert!(
                    (a - f64::from(b)).abs() <= 1e-5 * f64::from(b.abs()).max(1.0),
                    "node `{}`: {placed:?}",
                    node.name
                );
            }
        }
    }

    /// Prints what building a frame of the zone's world costs on the CPU, once the asset cache
    /// holds its meshes: the instances, the lights and the sky's environment that
    /// `slime-world-render` gives the renderer, which the game builds by hand today. Run it in a
    /// release build: `cargo test --release -p hunnawood -- --ignored --nocapture frame_of_the_zone`.
    #[test]
    #[ignore = "a measurement, which needs the cooked zone and a GPU"]
    fn what_building_a_frame_of_the_zone_s_world_costs() {
        use std::time::{Duration, Instant};

        use slime_gfx::{Gpu, TextureFormat};
        use slime_render::{AssetCache, Renderer};
        use slime_world::World;
        use slime_world_render::FrameBuilder;

        let (assets, _) = zone();
        let scene = assets.scene(assets::zone_id()).expect("the zone is cooked");
        let mut world = World::new();
        world.spawn_scene("zone", &scene).expect("spawned");
        world.update_transforms();
        let gpu = Gpu::new().expect("a GPU adapter must be available");
        let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
        let mut cache = AssetCache::new(assets);
        // Without a sky: its environment is made once, and is not what a frame costs.
        let mut builder = FrameBuilder::new(None);
        let started = Instant::now();
        loop {
            let frame = builder
                .build(&world, &mut cache, &renderer, &gpu, 0.0)
                .expect("the frame is built");
            assert!(frame.errors.is_empty(), "{:?}", frame.errors);
            if frame.loading == 0 {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "the zone still loads"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        eprintln!("the zone's meshes loaded in {:?}", started.elapsed());
        let mut times: Vec<Duration> = (0..200)
            .map(|_| {
                let started = Instant::now();
                let frame = builder
                    .build(&world, &mut cache, &renderer, &gpu, 0.5)
                    .expect("the frame is built");
                std::hint::black_box((frame.instances.len(), frame.lights.len()));
                started.elapsed()
            })
            .collect();
        times.sort_unstable();
        let frame = builder
            .build(&world, &mut cache, &renderer, &gpu, 0.5)
            .expect("the frame is built");
        eprintln!(
            "a frame of the zone's world, {} instances and {} lights: {:?} at best, {:?} at the median",
            frame.instances.len(),
            frame.lights.len(),
            times[0],
            times[times.len() / 2]
        );
    }
}
