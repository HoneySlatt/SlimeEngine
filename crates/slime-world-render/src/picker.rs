//! Picking by the CPU (`docs/adr/0130-picking.md`): a ray cast against the meshes that a world's
//! entities place, as the frame builder places them, without a GPU.

use std::collections::{HashMap, HashSet};

use slime_assets::{AssetId, CookedAssets};
use slime_render::glam::{DVec3, Vec3};
use slime_world::{Entity, Meshes, Transform, World};

/// Picks by the CPU: the entity whose mesh a ray meets first, as the world places it at a fraction
/// of a fixed step ([`World::interpolated_transform`]), as [`FrameBuilder`](crate::FrameBuilder)
/// draws it. It runs without a GPU: in tests, in the tools and agents that drive a game, and for a
/// game's own questions until physics casts rays of its own (ADR-0115, ADR-0130).
///
/// It reads each mesh from the cooked data it was made with, whole, the first time a ray is cast in
/// a world that names it, which costs the first cast the reading of every mesh the world names; it
/// keeps the box around the mesh's positions, and its triangles if the ray met the box, or else
/// reads it again when a later ray first meets it. It keeps both while the world names the mesh,
/// hidden or not, and drops them at the first cast after the world no longer does. A mesh that
/// cannot be read is left out, with a warning when it is first read.
///
/// A ray meets the triangles of both faces of a surface, whatever its material; it passes through
/// nothing that a masked material cuts out, which picking by the GPU does ([`slime_render::Pick`]),
/// and meets no entity hidden, nor one placed by a transform that flattens it.
#[derive(Debug)]
pub struct Picker {
    assets: CookedAssets,
    meshes: HashMap<AssetId, Read>,
}

/// What was read of a mesh: its box, `None` if it cannot be read, and its triangles once a ray met
/// the box.
#[derive(Debug)]
struct Read {
    bounds: Option<(Vec3, Vec3)>,
    triangles: Option<(Vec<Vec3>, Vec<u32>)>,
}

/// What a ray met first ([`Picker::cast`]): the entity, and how far along the ray, in metres from
/// where it starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// The entity that places the mesh met.
    pub entity: Entity,
    /// The distance from the ray's start.
    pub distance: f64,
}

impl Picker {
    /// A picker that reads meshes from `assets`, holding none yet.
    #[must_use]
    pub fn new(assets: CookedAssets) -> Self {
        Self {
            assets,
            meshes: HashMap::new(),
        }
    }

    /// The entity of `world` whose mesh the ray from `origin` along `direction`, of length one,
    /// meets first, where the world places it at `fraction` of a fixed step, as
    /// `slime-app`'s `GameTime::alpha` gives it; `None` if it meets none. A camera's
    /// [`ray`](slime_render::Camera::ray) through a pixel gives what is drawn there.
    pub fn cast(&mut self, world: &World, fraction: f64, ray: (DVec3, DVec3)) -> Option<Hit> {
        self.meet_entities(world, fraction, ray, true).pop()
    }

    /// Every entity of `world` whose mesh the ray meets, as [`Picker::cast`] places them, each
    /// once, where the ray first meets it, the nearest first: what lies behind what `cast` gives,
    /// through which an editor goes a click after another (ADR-0159). It reads every mesh whose box
    /// the ray meets, where `cast` leaves out those beyond the nearest that it met.
    pub fn cast_through(&mut self, world: &World, fraction: f64, ray: (DVec3, DVec3)) -> Vec<Hit> {
        let mut hits = self.meet_entities(world, fraction, ray, false);
        hits.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        hits
    }

    /// The entities that the ray meets, each with where it first meets it, in the world's order:
    /// the nearest alone if `nearest`.
    fn meet_entities(
        &mut self,
        world: &World,
        fraction: f64,
        (origin, direction): (DVec3, DVec3),
        nearest: bool,
    ) -> Vec<Hit> {
        let mut named = HashSet::new();
        let mut hits: Vec<Hit> = Vec::new();
        for (entity, (meshes, transform)) in world.query::<(&Meshes, &Transform)>() {
            named.extend(meshes.ids.iter().copied());
            if world.is_hidden(entity) {
                continue;
            }
            let Some(at) = world.interpolated_transform(entity, fraction) else {
                continue;
            };
            // The ray in the mesh's space, where a step along it is still a metre of the world's.
            let into_mesh = at.to_affine(transform.stretch).inverse();
            let (start, step) = (
                into_mesh.transform_point3(origin),
                into_mesh.transform_vector3(direction),
            );
            if !(start.is_finite() && step.is_finite()) {
                continue;
            }
            // The nearest that the entity's meshes met, or, for the nearest entity alone, than
            // the one met before.
            let mut met: Option<f64> = None;
            for &id in &meshes.ids {
                let before = hits.last().filter(|_| nearest).map(|hit| hit.distance);
                let bound = met.or(before).unwrap_or(f64::INFINITY);
                if let Some(distance) = self.meet(id, start, step, bound) {
                    met = Some(distance);
                }
            }
            if let Some(distance) = met {
                if nearest {
                    hits.clear();
                }
                hits.push(Hit { entity, distance });
            }
        }
        self.meshes.retain(|id, _| named.contains(id));
        hits
    }

    /// The box, in the world's axes, around the meshes of `entity` of `world` and of the entities
    /// below it, as [`Picker::cast`] places them, those hidden left out as a cast leaves them; none
    /// for an entity that places no mesh shown, or none that can be read. It reads each mesh's box
    /// from the cooked data the first time, as a cast does (ADR-0161).
    pub fn bounds(
        &mut self,
        world: &World,
        entity: Entity,
        fraction: f64,
    ) -> Option<(DVec3, DVec3)> {
        let mut joined: Option<(DVec3, DVec3)> = None;
        let mut stack = vec![entity];
        while let Some(each) = stack.pop() {
            if world.is_hidden(each) {
                continue;
            }
            stack.extend(world.children(each));
            let (Some(meshes), Some(transform), Some(at)) = (
                world.get::<Meshes>(each),
                world.get::<Transform>(each),
                world.interpolated_transform(each, fraction),
            ) else {
                continue;
            };
            let placed = at.to_affine(transform.stretch);
            for &id in &meshes.ids {
                let assets = &self.assets;
                let read = self.meshes.entry(id).or_insert_with(|| Read {
                    bounds: read(assets, id).map(|(bounds, _)| bounds),
                    triangles: None,
                });
                let Some((low, high)) = read.bounds else {
                    continue;
                };
                for corner in 0..8 {
                    let pick = |bit: u32, axis: usize| {
                        f64::from(if corner & bit == 0 {
                            low[axis]
                        } else {
                            high[axis]
                        })
                    };
                    let point =
                        placed.transform_point3(DVec3::new(pick(1, 0), pick(2, 1), pick(4, 2)));
                    joined = Some(joined.map_or((point, point), |(low, high)| {
                        (low.min(point), high.max(point))
                    }));
                }
            }
        }
        joined
    }

    /// How far along the ray from `start` by `step`, in the mesh's space, the mesh `id` is first
    /// met, if nearer than `nearest`; reading what it lacks of the mesh.
    fn meet(&mut self, id: AssetId, start: DVec3, step: DVec3, nearest: f64) -> Option<f64> {
        let meets = |(low, high): (Vec3, Vec3)| {
            through_box(start, step, low.as_dvec3(), high.as_dvec3())
                .is_some_and(|enter| enter < nearest)
        };
        let mesh = self.meshes.entry(id).or_insert_with(|| {
            let read = read(&self.assets, id);
            Read {
                bounds: read.as_ref().map(|(bounds, _)| *bounds),
                // Kept if this ray needs them, which a later one would read again.
                triangles: read
                    .filter(|(bounds, _)| meets(*bounds))
                    .map(|(_, triangles)| triangles),
            }
        });
        if !meets(mesh.bounds?) {
            return None;
        }
        if mesh.triangles.is_none() {
            mesh.triangles = read(&self.assets, id).map(|(_, triangles)| triangles);
        }
        let (positions, indices) = mesh.triangles.as_ref()?;
        indices
            .as_chunks::<3>()
            .0
            .iter()
            .filter_map(|&[a, b, c]| {
                let corner = |index: u32| positions.get(index as usize).map(|p| p.as_dvec3());
                through_triangle(start, step, [corner(a)?, corner(b)?, corner(c)?])
            })
            .filter(|&distance| distance < nearest)
            .min_by(f64::total_cmp)
    }
}

/// The box and the triangles of the mesh `id` of `assets`; `None`, with a warning, if it cannot be
/// read.
#[expect(
    clippy::type_complexity,
    reason = "a box and triangles, read together, kept apart"
)]
fn read(assets: &CookedAssets, id: AssetId) -> Option<((Vec3, Vec3), (Vec<Vec3>, Vec<u32>))> {
    match assets.mesh(id) {
        Ok(mesh) => {
            let [low, high] = mesh.bounds;
            let positions = mesh.positions.into_iter().map(Vec3::from).collect();
            Some(((low.into(), high.into()), (positions, mesh.indices)))
        }
        Err(error) => {
            tracing::warn!(%id, "a mesh is left out of picking by the CPU: {error}");
            None
        }
    }
}

/// How far along the ray from `start` by `step` it enters the box from `low` to `high`, 0 if it
/// starts inside; `None` if it misses the box, or meets it only behind its start.
fn through_box(start: DVec3, step: DVec3, low: DVec3, high: DVec3) -> Option<f64> {
    let (mut enter, mut leave) = (0.0_f64, f64::INFINITY);
    for axis in 0..3 {
        let (from, by) = (start[axis], step[axis]);
        if by == 0.0 {
            if from < low[axis] || from > high[axis] {
                return None;
            }
            continue;
        }
        let (first, second) = ((low[axis] - from) / by, (high[axis] - from) / by);
        enter = enter.max(first.min(second));
        leave = leave.min(first.max(second));
    }
    (enter <= leave).then_some(enter)
}

/// How far along the ray from `start` by `step` it meets the triangle of `corners`, from either
/// face (Möller and Trumbore's test); `None` if it misses it, runs along its plane, or meets it
/// behind its start.
fn through_triangle(start: DVec3, step: DVec3, [a, b, c]: [DVec3; 3]) -> Option<f64> {
    let (ab, ac) = (b - a, c - a);
    let p = step.cross(ac);
    let determinant = ab.dot(p);
    // Along the plane, but for rounding, relative to the sizes involved: a tiny triangle of a mesh
    // scaled up is met all the same.
    if determinant.abs() <= f64::EPSILON * ab.length() * ac.length() * step.length() {
        return None;
    }
    let to_start = start - a;
    let u = to_start.dot(p) / determinant;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = to_start.cross(ab);
    let v = step.dot(q) / determinant;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = ac.dot(q) / determinant;
    (distance >= 0.0).then_some(distance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ray_enters_a_box_where_it_crosses_its_nearest_face_and_misses_one_beside_it() {
        let (low, high) = (DVec3::splat(-1.0), DVec3::splat(1.0));
        let along_z = DVec3::new(0.0, 0.0, -1.0);
        assert_eq!(
            through_box(DVec3::new(0.5, 0.0, 5.0), along_z, low, high),
            Some(4.0)
        );
        assert_eq!(through_box(DVec3::ZERO, along_z, low, high), Some(0.0));
        assert_eq!(
            through_box(DVec3::new(2.0, 0.0, 5.0), along_z, low, high),
            None
        );
        assert_eq!(
            through_box(DVec3::new(0.0, 0.0, -5.0), along_z, low, high),
            None,
            "behind its start"
        );
    }

    #[test]
    fn a_ray_meets_a_triangle_from_either_face_inside_its_edges_and_not_beyond() {
        let triangle = [
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(0.0, 2.0, 0.0),
        ];
        let down = DVec3::new(0.0, 0.0, -1.0);
        assert_eq!(
            through_triangle(DVec3::new(0.5, 0.5, 3.0), down, triangle),
            Some(3.0)
        );
        assert_eq!(
            through_triangle(DVec3::new(0.5, 0.5, -3.0), -down, triangle),
            Some(3.0),
            "from behind"
        );
        assert_eq!(
            through_triangle(DVec3::new(1.5, 1.5, 3.0), down, triangle),
            None
        );
        assert_eq!(
            through_triangle(DVec3::new(0.5, 0.5, 3.0), DVec3::X, triangle),
            None,
            "along its plane"
        );
    }
}
