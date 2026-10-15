//! The place the game is played in: its ground, the colliders that stop the hero, and where the
//! hero starts. Positions are x and z in metres, on the ground, at y = 0, +y up; north is −z and
//! east +x; an angle about the vertical is in radians from +x towards +z.
//!
//! Until the zone is made from its sources and cooked (Stage 4a, M4a.6), the game is played on a
//! test ground built here, with the two kinds of collider that the zone will have.

use slime_render::glam::Vec2;

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
    fn turned_box(center: [f32; 2], size: [f32; 2], yaw: f32, height: f32) -> Self {
        Self {
            shape: Shape::Box {
                half_size: Vec2::from(size) / 2.0,
                yaw,
            },
            center: Vec2::from(center),
            height,
        }
    }

    fn cylinder(center: [f32; 2], radius: f32, height: f32) -> Self {
        Self {
            shape: Shape::Cylinder { radius },
            center: Vec2::from(center),
            height,
        }
    }

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

/// A zone: its ground, its colliders, and where the hero starts.
#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    /// Half the size of the ground along x and z, around the origin; the hero does not leave it.
    pub half_size: Vec2,
    pub colliders: Vec<Collider>,
    /// Where the hero starts.
    pub start: Vec2,
    /// Which way the hero and the camera look at the start, of length 1.
    pub facing: Vec2,
}

impl Zone {
    /// The test ground: 40 m square, with the pieces of a ruined wall and a fallen block turned at
    /// angles, a colonnade of four pillars, and the trunks of a few trees. The hero starts in the
    /// south, looking north, at the ruins.
    pub fn test_ground() -> Self {
        let wall = Collider::turned_box;
        let colliders = vec![
            // The ruined wall: three pieces of different heights, turned off its line.
            wall([-6.0, -4.0], [4.0, 0.6], 0.15, 2.2),
            wall([-1.5, -5.0], [3.0, 0.6], -0.3, 1.2),
            wall([5.0, -3.5], [5.0, 0.7], 0.5, 3.0),
            // A block fallen from it, lower than the camera.
            wall([3.0, 4.0], [1.6, 1.2], 0.8, 0.9),
            // The colonnade, beyond the wall.
            Collider::cylinder([-3.0, -9.0], 0.45, 4.0),
            Collider::cylinder([3.0, -9.0], 0.45, 4.0),
            Collider::cylinder([-3.0, -14.0], 0.45, 4.0),
            Collider::cylinder([3.0, -14.0], 0.45, 4.0),
            // Trees.
            Collider::cylinder([-9.0, 6.0], 0.6, 8.0),
            Collider::cylinder([8.0, 9.0], 0.7, 9.0),
            Collider::cylinder([-12.0, -8.0], 0.8, 10.0),
            Collider::cylinder([11.0, -12.0], 0.6, 8.0),
            Collider::cylinder([-5.0, 14.0], 0.5, 7.0),
        ];
        Self {
            half_size: Vec2::splat(20.0),
            colliders,
            start: Vec2::new(0.0, 12.0),
            facing: Vec2::NEG_Y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_is_on_the_ground_and_clear_of_every_collider() {
        let zone = Zone::test_ground();
        assert!(zone.start.abs().cmplt(zone.half_size - 1.0).all());
        for collider in &zone.colliders {
            assert!(!collider.covers(zone.start, 1.0), "{collider:?}");
        }
        assert!((zone.facing.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn every_collider_stands_on_the_ground() {
        let zone = Zone::test_ground();
        for collider in &zone.colliders {
            assert!(
                collider.center.abs().cmplt(zone.half_size - 2.0).all(),
                "{collider:?}"
            );
            assert!(collider.height > 0.0);
        }
    }

    #[test]
    fn a_turned_box_covers_its_turned_footprint() {
        // 4 m long along its first axis, turned a quarter turn: it lies along z.
        let collider =
            Collider::turned_box([0.0, 0.0], [4.0, 1.0], std::f32::consts::FRAC_PI_2, 1.0);
        assert!(collider.covers(Vec2::new(0.0, 1.9), 0.0));
        assert!(!collider.covers(Vec2::new(1.9, 0.0), 0.0));
        assert!(collider.covers(Vec2::new(0.7, 0.0), 0.25));
        assert!(!collider.covers(Vec2::new(0.8, 0.0), 0.25));
    }
}

/// The zone as the cooked data holds it (`games/hunnawood/zone/build_zone.py` makes it, the cooker
/// cooks it): its colliders, markers and lights, read from the extras of the scene's nodes. Only the
/// tests read it until the game loads its zone from the cooked data.
#[cfg(test)]
mod cooked {
    use std::collections::{HashSet, VecDeque};
    use std::str::FromStr;

    use slime_assets::{
        AssetId, AssetKind, CookedAssets, CookedScene, Extra, LightKind, SceneNode,
    };
    use slime_render::glam::{Mat4, Vec2};

    use super::{Collider, Shape, Zone};
    use crate::rules::HERO_RADIUS;

    const SKIP: &str = "needs the cooked zone: scripts/fetch-assets.sh zone/, then cargo run --release -p \
                        slime-cook -- cook games/hunnawood/assets/zone target/cooked/hunnawood";

    /// The zone's places, beside what `Zone` holds.
    struct Places {
        zone: Zone,
        item: Vec2,
        door: Vec2,
        campfire: Vec2,
        terrace: Vec2,
        /// The colliders under the door's marker, which open with it; `zone.colliders` holds them
        /// too.
        door_colliders: Vec<Collider>,
    }

    fn assets() -> CookedAssets {
        CookedAssets::open(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/cooked/hunnawood"
        ))
        .unwrap_or_else(|error| panic!("{error}; the test {SKIP}"))
    }

    /// The ID of the zone's file, which its default scene has.
    fn zone_id() -> AssetId {
        let meta = include_str!("../assets/zone/hunna_wood.gltf.meta");
        let id = meta
            .trim()
            .trim_start_matches("id = \"")
            .trim_end_matches('"');
        AssetId::from_str(id).expect("the metadata file holds an ID")
    }

    fn text<'a>(node: &'a SceneNode, name: &str) -> Option<&'a str> {
        node.extras
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, e)| match e {
                Extra::Text(text) => Some(text.as_str()),
                _ => None,
            })
    }

    fn number(node: &SceneNode, name: &str) -> Option<f32> {
        node.extras
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, e)| match e {
                Extra::Number(number) => Some(*number as f32),
                _ => None,
            })
    }

    /// Whether node `index` is `ancestor` or below it.
    fn under(scene: &CookedScene, mut index: usize, ancestor: usize) -> bool {
        loop {
            if index == ancestor {
                return true;
            }
            match scene.nodes[index].parent {
                Some(parent) => index = parent as usize,
                None => return false,
            }
        }
    }

    fn places(scene: &CookedScene) -> Places {
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
            assert_eq!(found.len(), 1, "markers `{kind}`: {found:?}");
            found[0]
        };
        let door = marker("door");
        let mut colliders = Vec::new();
        let mut door_colliders = Vec::new();
        for (i, node) in scene.nodes.iter().enumerate() {
            let Some(kind) = text(node, "collider") else {
                continue;
            };
            let m = world[i];
            let height = number(node, "height").expect("a collider has a height");
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
                other => panic!("node {}: collider `{other}`", node.name),
            };
            let collider = Collider {
                shape,
                center: ground(&m),
                height,
            };
            colliders.push(collider);
            if under(scene, i, door) {
                door_colliders.push(collider);
            }
        }
        let start = world[marker("start")];
        let bounds = world[marker("bounds")];
        let facing = Vec2::new(-start.z_axis.x, -start.z_axis.z).normalize();
        Places {
            zone: Zone {
                half_size: Vec2::new(
                    bounds.x_axis.truncate().length(),
                    bounds.z_axis.truncate().length(),
                ),
                colliders,
                start: ground(&start),
                facing,
            },
            item: ground(&world[marker("item")]),
            door: ground(&world[door]),
            campfire: ground(&world[marker("campfire")]),
            terrace: ground(&world[marker("terrace")]),
            door_colliders,
        }
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
        let places = places(&assets().scene(zone_id()).expect(SKIP));
        let zone = &places.zone;
        assert!(zone.start.abs().cmplt(zone.half_size - 1.0).all());
        for collider in &zone.colliders {
            assert!(!collider.covers(zone.start, HERO_RADIUS), "{collider:?}");
        }
        assert!((zone.facing.length() - 1.0).abs() < 1e-4);
        assert!(!places.door_colliders.is_empty(), "the door has a collider");
        for place in [places.item, places.door, places.campfire, places.terrace] {
            assert!(place.abs().cmplt(zone.half_size).all(), "{place}");
        }
    }

    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_hero_reaches_the_item_and_the_campfire_and_the_door_keeps_the_terrace() {
        let places = places(&assets().scene(zone_id()).expect(SKIP));
        let closed = Reach::new(&places.zone, &places.zone.colliders);
        assert!(closed.near(places.item, 1.2), "the item is out of reach");
        assert!(
            closed.near(places.campfire, 2.0),
            "the campfire is out of reach"
        );
        assert!(closed.near(places.door, 1.5), "the door is out of reach");
        assert!(
            !closed.near(places.terrace, 2.0),
            "the terrace is reached without the door"
        );
        // Once the door opens, the terrace is reached through it.
        let open: Vec<Collider> = places
            .zone
            .colliders
            .iter()
            .filter(|c| !places.door_colliders.contains(c))
            .copied()
            .collect();
        assert!(
            Reach::new(&places.zone, &open).near(places.terrace, 1.0),
            "the open door leads nowhere"
        );
    }

    /// The zone's budgets (`docs/PLAN.md`, M4a.6): what the renderer draws in every frame, the
    /// lights that cast shadows at night, the GPU memory of its textures and the size of its
    /// cooked data.
    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_cooked_zone_keeps_within_its_budgets() {
        let assets = assets();
        let scene = assets.scene(zone_id()).expect(SKIP);
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
        let mut stack = vec![zone_id()];
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
}
