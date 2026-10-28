//! The handles that move, turn and scale what is selected
//! (`docs/adr/0161-the-open-step-and-the-handles.md`): where they stand, about the selection's
//! pivot, in the world's axes or the selection's; which part of them a ray meets, by the CPU, at
//! once; and the commands that a drag of a part gives, at each frame, snapped if asked, which the
//! core applies live as an open step. It needs no GPU: the viewport draws the handles from what
//! this module says.

use std::f64::consts::TAU;

use slime_reflect::{Path, Reflect};
use slime_render::glam::{DAffine3, DQuat, DVec3, Vec3};
use slime_render::{Camera, MeshData, Projection};
use slime_world::{Command, Entity, GlobalTransform, PersistentId, Transform, World};

/// How long the handles show, from the pivot to an arrow's tip, in logical pixels, whatever the
/// camera's distance.
const SIZE: f64 = 100.0;

/// How near a ray passes to an arrow's shaft, or to a ring, to meet it, as a share of the
/// handles' size.
const REACH: f64 = 0.08;

/// Where the planes' squares lie along their two axes, and their side, as shares of the size.
const PLANE_AT: f64 = 0.3;
const PLANE_SIDE: f64 = 0.2;

/// The side of the cube in the middle of the scale's handles, as a share of the size.
const CENTER: f64 = 0.15;

/// The steps that snapping moves, turns and scales by: a quarter of a metre on the world's grid,
/// fifteen degrees, and a tenth of the size.
pub(crate) const SNAP_DISTANCE: f64 = 0.25;
pub(crate) const SNAP_ANGLE: f64 = TAU / 24.0;
pub(crate) const SNAP_SCALE: f64 = 0.1;

/// What the handles do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tool {
    /// Move, along an axis or in a plane.
    #[default]
    Move,
    /// Turn, about an axis.
    Turn,
    /// Scale: along an axis, the selection's stretch; from the middle, its scale.
    Scale,
}

/// The axes that the handles follow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Space {
    /// The world's.
    #[default]
    World,
    /// Those of the last entity selected, as it is turned.
    Selection,
}

/// A part of the handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    /// The arrow, or the scale's handle, along the axis of this index: X, Y, Z.
    Axis(usize),
    /// The square of the plane across the axis of this index, which moves in that plane.
    Plane(usize),
    /// The ring about the axis of this index.
    Ring(usize),
    /// The scale's middle, which scales whole.
    Center,
}

/// Where the handles stand: their pivot, the middle of what is selected, how they are turned,
/// their three axes, and their size in metres, which keeps them [`SIZE`] logical pixels long on the
/// screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Gizmo {
    pub(crate) pivot: DVec3,
    pub(crate) rotation: DQuat,
    pub(crate) axes: [DVec3; 3],
    pub(crate) size: f64,
}

impl Gizmo {
    /// Where the handles of `selection`, entities of `world`, stand in `space`, seen by `camera`
    /// in a viewport `height` logical pixels high: about the middle of what they move
    /// ([`outermost`]); none for a selection of nothing placed.
    pub(crate) fn of(
        world: &World,
        selection: &[Entity],
        space: Space,
        camera: &Camera,
        height: f32,
    ) -> Option<Self> {
        let placed: Vec<GlobalTransform> = outermost(world, selection)
            .into_iter()
            .filter_map(|entity| world.interpolated_transform(entity, 1.0))
            .collect();
        // The last entity selected, whether or not it is among those moved.
        let last = selection
            .iter()
            .rev()
            .find_map(|&entity| world.interpolated_transform(entity, 1.0))?;
        let pivot = placed
            .iter()
            .map(|placed| placed.translation)
            .sum::<DVec3>()
            / placed.len() as f64;
        let rotation = match space {
            Space::World => DQuat::IDENTITY,
            Space::Selection => last.rotation.as_dquat(),
        };
        let axes = [DVec3::X, DVec3::Y, DVec3::Z].map(|axis| rotation * axis);
        // The length that SIZE pixels cover at the pivot's distance.
        let across = match camera.projection {
            Projection::Perspective { fov_y } => {
                2.0 * camera.eye.distance(pivot) * (f64::from(fov_y) / 2.0).tan()
            }
            Projection::Orthographic { height, .. } => f64::from(height),
        };
        let size = across * SIZE / f64::from(height.max(1.0));
        Some(Self {
            pivot,
            rotation,
            axes,
            size,
        })
    }

    /// The part of the handles of `tool` that the ray from `origin` along the unit `direction`
    /// meets first, if any.
    pub(crate) fn hit(&self, tool: Tool, (origin, direction): (DVec3, DVec3)) -> Option<Part> {
        let mut nearest: Option<(f64, Part)> = None;
        let mut meet = |distance: Option<f64>, part: Part| {
            if let Some(distance) = distance
                && nearest.is_none_or(|(best, _)| distance < best)
            {
                nearest = Some((distance, part));
            }
        };
        let reach = self.size * REACH;
        for (index, &axis) in self.axes.iter().enumerate() {
            match tool {
                // From beyond the middle, which the scale's cube holds.
                Tool::Move | Tool::Scale => {
                    let base = self.pivot + axis * self.size * CENTER;
                    let tip = self.pivot + axis * self.size;
                    meet(
                        segment(origin, direction, base, tip, reach),
                        Part::Axis(index),
                    );
                }
                Tool::Turn => meet(
                    ring(origin, direction, self.pivot, axis, self.size, reach),
                    Part::Ring(index),
                ),
            }
            if tool == Tool::Move {
                let [u, v] = others(index).map(|other| self.axes[other]);
                let middle = self.pivot + (u + v) * self.size * (PLANE_AT + PLANE_SIDE / 2.0);
                let half = self.size * PLANE_SIDE / 2.0;
                meet(
                    square(origin, direction, middle, [u, v], half),
                    Part::Plane(index),
                );
            }
        }
        if tool == Tool::Scale {
            // The cube in the middle, met as the ball around it.
            let half = self.size * CENTER / 2.0;
            meet(
                ball(origin, direction, self.pivot, half * 1.5),
                Part::Center,
            );
        }
        nearest.map(|(_, part)| part)
    }
}

/// What of `selection`, entities of `world`, a gesture moves: each entity without an ancestor in
/// it, which carries its children along, so that none moves twice.
pub(crate) fn outermost(world: &World, selection: &[Entity]) -> Vec<Entity> {
    selection
        .iter()
        .copied()
        .filter(|&entity| {
            std::iter::successors(world.parent(entity), |&each| world.parent(each))
                .all(|ancestor| !selection.contains(&ancestor))
        })
        .collect()
}

/// The two axes other than `index`, in their order.
fn others(index: usize) -> [usize; 2] {
    [(index + 1) % 3, (index + 2) % 3]
}

/// How far along the ray it passes within `reach` of the segment from `from` to `to`, if it does,
/// in front of its start.
fn segment(origin: DVec3, direction: DVec3, from: DVec3, to: DVec3, reach: f64) -> Option<f64> {
    let (along, t) = closest(origin, direction, from, (to - from).normalize_or_zero())?;
    let length = from.distance(to);
    if !(0.0..=length).contains(&t) || along < 0.0 {
        return None;
    }
    let on_ray = origin + direction * along;
    let on_segment = from + (to - from).normalize_or_zero() * t;
    (on_ray.distance(on_segment) <= reach).then_some(along)
}

/// The parameters of the points where the ray and the line through `point` along the unit `axis`
/// come nearest: along the ray, and along the line; none for parallel ones.
fn closest(origin: DVec3, direction: DVec3, point: DVec3, axis: DVec3) -> Option<(f64, f64)> {
    let between = origin - point;
    let b = direction.dot(axis);
    let denominator = 1.0 - b * b;
    if denominator.abs() < 1e-9 {
        return None;
    }
    let (d, e) = (direction.dot(between), axis.dot(between));
    let along = (b * e - d) / denominator;
    let t = (e - b * d) / denominator;
    Some((along, t))
}

/// Where along the ray it meets the plane through `point` across `normal`, in front of its start.
fn plane(origin: DVec3, direction: DVec3, point: DVec3, normal: DVec3) -> Option<f64> {
    let facing = direction.dot(normal);
    if facing.abs() < 1e-9 {
        return None;
    }
    let along = (point - origin).dot(normal) / facing;
    (along >= 0.0).then_some(along)
}

/// How far along the ray it meets the ring of `radius` about `axis` through `center`, within
/// `reach` of it.
fn ring(
    origin: DVec3,
    direction: DVec3,
    center: DVec3,
    axis: DVec3,
    radius: f64,
    reach: f64,
) -> Option<f64> {
    let along = plane(origin, direction, center, axis)?;
    let distance = (origin + direction * along).distance(center);
    ((distance - radius).abs() <= reach).then_some(along)
}

/// How far along the ray it meets the square about `middle` along the axes `u` and `v`, of half
/// side `half`.
fn square(
    origin: DVec3,
    direction: DVec3,
    middle: DVec3,
    [u, v]: [DVec3; 2],
    half: f64,
) -> Option<f64> {
    let along = plane(origin, direction, middle, u.cross(v))?;
    let offset = origin + direction * along - middle;
    (offset.dot(u).abs() <= half && offset.dot(v).abs() <= half).then_some(along)
}

/// How far along the ray it meets the ball about `center` of `radius`.
fn ball(origin: DVec3, direction: DVec3, center: DVec3, radius: f64) -> Option<f64> {
    let along = (center - origin).dot(direction);
    let nearest = origin + direction * along;
    (along >= 0.0 && nearest.distance(center) <= radius).then_some(along)
}

/// An entity that a drag moves, and where it was as the drag started.
#[derive(Clone, Debug)]
struct Held {
    id: PersistentId,
    /// Its own transform, and where it was in the world.
    local: Transform,
    global: GlobalTransform,
    /// Where its parent is in the world, which its own transform is relative to.
    parent: GlobalTransform,
}

/// A part of the handles dragged: what it started from, and what the selection was.
#[derive(Clone, Debug)]
pub(crate) struct Drag {
    tool: Tool,
    part: Part,
    gizmo: Gizmo,
    /// Where the drag started on the part: a point on its axis or plane, or the direction of the
    /// start on a ring's plane, from the pivot.
    from: DVec3,
    held: Vec<Held>,
}

impl Drag {
    /// A drag of `part` of the handles of `tool` that stand as `gizmo` says, started where the
    /// ray `ray` meets it, of the [`outermost`] of `selection`, entities of `world`; none if the
    /// ray does not reach what the part moves along, or nothing selected is placed and named.
    pub(crate) fn start(
        world: &World,
        selection: &[Entity],
        (tool, part, gizmo): (Tool, Part, Gizmo),
        ray: (DVec3, DVec3),
    ) -> Option<Self> {
        let held: Vec<Held> = outermost(world, selection)
            .into_iter()
            .filter_map(|entity| {
                let parent = world
                    .parent(entity)
                    .and_then(|parent| world.interpolated_transform(parent, 1.0))
                    .unwrap_or(GlobalTransform::IDENTITY);
                Some(Held {
                    id: world.persistent_id(entity)?,
                    local: *world.get::<Transform>(entity)?,
                    global: world.interpolated_transform(entity, 1.0)?,
                    parent,
                })
            })
            .collect();
        if held.is_empty() {
            return None;
        }
        let mut drag = Self {
            tool,
            part,
            gizmo,
            from: DVec3::ZERO,
            held,
        };
        drag.from = drag.reach(ray)?;
        Some(drag)
    }

    /// The part dragged.
    pub(crate) fn part(&self) -> Part {
        self.part
    }

    /// What the step that the drag makes is called.
    pub(crate) fn label(&self) -> &'static str {
        match self.tool {
            Tool::Move => "Move",
            Tool::Turn => "Turn",
            Tool::Scale => "Scale",
        }
    }

    /// Where `ray` reaches what the part moves along: a point on its axis or its plane, or, on a
    /// ring's plane, the direction from the pivot.
    fn reach(&self, (origin, direction): (DVec3, DVec3)) -> Option<DVec3> {
        let Gizmo { pivot, axes, .. } = self.gizmo;
        match self.part {
            Part::Axis(index) => {
                let (_, t) = closest(origin, direction, pivot, axes[index])?;
                Some(pivot + axes[index] * t)
            }
            Part::Plane(index) => {
                let along = plane(origin, direction, pivot, axes[index])?;
                Some(origin + direction * along)
            }
            Part::Ring(index) => {
                let along = plane(origin, direction, pivot, axes[index])?;
                Some((origin + direction * along - pivot).normalize_or_zero())
            }
            // The scale's middle scales by how far the ray comes from the pivot, across the view.
            Part::Center => {
                let along = (pivot - origin).dot(direction);
                Some(origin + direction * along)
            }
        }
    }

    /// The commands that put each entity dragged where the ray `ray` takes it, snapped to the
    /// steps of [`SNAP_DISTANCE`], [`SNAP_ANGLE`] and [`SNAP_SCALE`] if `snap`; each sets the
    /// entity's whole transform. None while the ray reaches nothing.
    pub(crate) fn commands(&self, ray: (DVec3, DVec3), snap: bool) -> Vec<Command> {
        let Some(to) = self.reach(ray) else {
            return Vec::new();
        };
        let Gizmo { pivot, axes, .. } = self.gizmo;
        let moved = |held: &Held| {
            let mut local = held.local;
            match (self.tool, self.part) {
                (Tool::Move, _) => {
                    let mut delta = to - self.from;
                    if snap {
                        delta = snapped(&self.gizmo, self.part, delta);
                    }
                    let world = held.global.translation + delta;
                    local.translation = into_parent(&held.parent, world);
                }
                (Tool::Turn, Part::Ring(index)) => {
                    let axis = axes[index];
                    let mut angle = self.from.cross(to).dot(axis).atan2(self.from.dot(to));
                    if snap {
                        angle = (angle / SNAP_ANGLE).round() * SNAP_ANGLE;
                    }
                    let turn = DQuat::from_axis_angle(axis, angle);
                    let world = pivot + turn * (held.global.translation - pivot);
                    local.translation = into_parent(&held.parent, world);
                    let rotation = turn * held.global.rotation.as_dquat();
                    local.rotation = (held.parent.rotation.as_dquat().inverse() * rotation)
                        .normalize()
                        .as_quat();
                }
                (Tool::Scale, part) => {
                    let (from, to) = (self.from.distance(pivot), to.distance(pivot));
                    let scale = match part {
                        // Along an axis, by how much farther from the pivot the stick's end is.
                        Part::Axis(_) if from > f64::EPSILON => to / from,
                        Part::Axis(_) => 1.0,
                        // From the middle, by how far the pointer moved away from it, the handles'
                        // size doubling: a press near the pivot is no tiny distance to divide by.
                        _ => 1.0 + (to - from) / self.gizmo.size,
                    };
                    let scale = if snap {
                        (scale / SNAP_SCALE).round() * SNAP_SCALE
                    } else {
                        scale
                    };
                    let scale = scale.max(0.01);
                    match part {
                        // Along an axis: the entity's stretch along its own axis nearest it,
                        // which no child inherits.
                        Part::Axis(index) => {
                            let own = held.global.rotation.as_dquat().inverse() * axes[index];
                            let along = own.abs().max_position();
                            let mut stretch = held.local.stretch;
                            stretch[along] = (f64::from(stretch[along]) * scale) as f32;
                            local.stretch = stretch;
                        }
                        // Whole: its scale, and where it stands from the pivot.
                        _ => {
                            local.scale = (f64::from(held.local.scale) * scale) as f32;
                            let world = pivot + (held.global.translation - pivot) * scale;
                            local.translation = into_parent(&held.parent, world);
                        }
                    }
                }
                (Tool::Turn, _) => {}
            }
            local
        };
        self.held
            .iter()
            .map(|held| Command::SetProperty {
                entity: held.id.clone(),
                component: "Transform".to_owned(),
                path: Path::default(),
                value: moved(held).to_value(),
            })
            .collect()
    }
}

/// `delta`, a move of the pivot of `gizmo` along the axis or in the plane of `part`, snapped by
/// [`SNAP_DISTANCE`] along the axes that it moves along, and along those alone: in the world's axes,
/// so that the pivot lands on the world's grid; in the selection's, by steps from where it started,
/// as no grid of the world lies along them.
fn snapped(gizmo: &Gizmo, part: Part, delta: DVec3) -> DVec3 {
    let along = |index: usize| {
        let axis = gizmo.axes[index];
        let moved = delta.dot(axis);
        let snapped = if gizmo.rotation == DQuat::IDENTITY {
            let start = gizmo.pivot.dot(axis);
            ((start + moved) / SNAP_DISTANCE).round() * SNAP_DISTANCE - start
        } else {
            (moved / SNAP_DISTANCE).round() * SNAP_DISTANCE
        };
        axis * snapped
    };
    match part {
        Part::Axis(index) => along(index),
        Part::Plane(index) => others(index).into_iter().map(along).sum(),
        Part::Ring(_) | Part::Center => delta,
    }
}

/// `world`, a point in the world, in the space of a parent that stands as `parent` says.
pub(crate) fn into_parent(parent: &GlobalTransform, world: DVec3) -> DVec3 {
    let scale = f64::from(parent.scale).max(f64::EPSILON);
    parent.rotation.as_dquat().inverse() * (world - parent.translation) / scale
}

/// The transform of the arrow, ring or square of `part` of handles that stand as `gizmo` says, for
/// the viewport's meshes, which lie along +Y from the origin, a unit long, or about +Y: turned as
/// the handles are, so that a square's sides and a cube's faces follow their axes.
pub(crate) fn placed(gizmo: &Gizmo, part: Part) -> DAffine3 {
    // Turned from +Y to the axis of `index`, then as the handles are.
    let up = |index: usize| {
        let axis = [DVec3::X, DVec3::Y, DVec3::Z][index];
        gizmo.rotation * DQuat::from_rotation_arc(DVec3::Y, axis)
    };
    match part {
        Part::Axis(index) | Part::Ring(index) => DAffine3::from_scale_rotation_translation(
            DVec3::splat(gizmo.size),
            up(index),
            gizmo.pivot,
        ),
        Part::Plane(index) => {
            let [u, v] = others(index).map(|other| gizmo.axes[other]);
            let middle = gizmo.pivot + (u + v) * gizmo.size * (PLANE_AT + PLANE_SIDE / 2.0);
            DAffine3::from_scale_rotation_translation(
                DVec3::splat(gizmo.size * PLANE_SIDE),
                up(index),
                middle,
            )
        }
        Part::Center => DAffine3::from_scale_rotation_translation(
            DVec3::splat(gizmo.size * CENTER),
            gizmo.rotation,
            gizmo.pivot,
        ),
    }
}

/// The meshes of the handles, as [`placed`] places them: an arrow a unit long along +Y, a stick
/// ending in a cube for the scale's axes, a ring of radius one about +Y, and a square of side one
/// across +Y.
pub(crate) struct Meshes {
    pub(crate) arrow: MeshData,
    pub(crate) stick: MeshData,
    pub(crate) ring: MeshData,
    pub(crate) square: MeshData,
}

impl Meshes {
    pub(crate) fn new() -> Self {
        let mut arrow = tube(0.0, 0.82, 0.015, 0.015);
        append(&mut arrow, tube(0.82, 1.0, 0.055, 0.0));
        let mut stick = tube(0.0, 0.9, 0.015, 0.015);
        let mut tip = MeshData::cube();
        for position in &mut tip.positions {
            *position = [
                position[0] * 0.1,
                position[1] * 0.1 + 0.95,
                position[2] * 0.1,
            ];
        }
        append(&mut stick, tip);
        Self {
            arrow,
            stick,
            ring: torus(1.0, 0.015),
            square: across_y(),
        }
    }
}

/// How many sides the round parts of the handles have.
const SIDES: u32 = 24;

/// A tube along +Y from `from` to `to`, of radius `bottom` there and `top` here: a cylinder, or a
/// cone ending in a point, closed at its bottom.
fn tube(from: f32, to: f32, bottom: f32, top: f32) -> MeshData {
    let mut mesh = empty();
    for side in 0..=SIDES {
        let angle = side as f32 / SIDES as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        // The slope of the side, which tilts its normal.
        let slope = (bottom - top) / (to - from);
        let normal = Vec3::new(cos, slope, sin).normalize();
        for (height, radius) in [(from, bottom), (to, top)] {
            mesh.positions.push([cos * radius, height, sin * radius]);
            mesh.normals.push(normal.to_array());
            mesh.uvs.push([side as f32 / SIDES as f32, 0.0]);
            mesh.tangents.push([-sin, 0.0, cos, 1.0]);
        }
    }
    for side in 0..SIDES {
        let first = side * 2;
        mesh.indices
            .extend([first, first + 1, first + 3, first, first + 3, first + 2]);
    }
    // Its bottom, a fan about its middle.
    let middle = mesh.positions.len() as u32;
    mesh.positions.push([0.0, from, 0.0]);
    mesh.normals.push([0.0, -1.0, 0.0]);
    mesh.uvs.push([0.0, 0.0]);
    mesh.tangents.push([1.0, 0.0, 0.0, 1.0]);
    for side in 0..SIDES {
        mesh.indices.extend([middle, side * 2 + 2, side * 2]);
    }
    mesh
}

/// A ring about +Y of `radius`, its tube of radius `thickness`.
fn torus(radius: f32, thickness: f32) -> MeshData {
    let mut mesh = empty();
    let around = SIDES * 2;
    for step in 0..=around {
        let angle = step as f32 / around as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        let out = Vec3::new(cos, 0.0, sin);
        for side in 0..=8_u32 {
            let tube = side as f32 / 8.0 * std::f32::consts::TAU;
            let normal = out * tube.cos() + Vec3::Y * tube.sin();
            let position = out * radius + normal * thickness;
            mesh.positions.push(position.to_array());
            mesh.normals.push(normal.to_array());
            mesh.uvs
                .push([step as f32 / around as f32, side as f32 / 8.0]);
            mesh.tangents.push([-sin, 0.0, cos, 1.0]);
        }
    }
    for step in 0..around {
        for side in 0..8 {
            let at = |step: u32, side: u32| step * 9 + side;
            let (a, b, c, d) = (
                at(step, side),
                at(step + 1, side),
                at(step + 1, side + 1),
                at(step, side + 1),
            );
            mesh.indices.extend([a, c, b, a, d, c]);
        }
    }
    mesh
}

/// A square of side one across +Y, about the origin.
fn across_y() -> MeshData {
    let mut square = MeshData::square();
    for position in &mut square.positions {
        *position = [position[0], 0.0, -position[1]];
    }
    square.normals = vec![[0.0, 1.0, 0.0]; 4];
    square
}

/// A mesh of nothing, to build on.
fn empty() -> MeshData {
    MeshData {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        tangents: Vec::new(),
        indices: Vec::new(),
    }
}

/// Adds `more`'s triangles to `mesh`'s.
fn append(mesh: &mut MeshData, more: MeshData) {
    let offset = mesh.positions.len() as u32;
    mesh.positions.extend(more.positions);
    mesh.normals.extend(more.normals);
    mesh.uvs.extend(more.uvs);
    mesh.tangents.extend(more.tangents);
    mesh.indices
        .extend(more.indices.into_iter().map(|index| index + offset));
}

#[cfg(test)]
mod tests {
    use slime_render::glam::Quat;

    use super::*;

    fn near(a: DVec3, b: DVec3) -> bool {
        a.distance(b) < 1e-6
    }

    /// A world of two entities, keyed `a` at (1, 0, 0) and `b` at (3, 0, 0), and `child`, a child
    /// of `b` turned a quarter about Y, at (0, 0, 1) from it.
    fn world() -> (World, [Entity; 3]) {
        let mut world = World::new();
        let mut placed = |key: &str, translation: DVec3, rotation: Quat| {
            let entity = world.spawn_keyed(key).expect("a key");
            world.insert(
                entity,
                Transform {
                    translation,
                    rotation,
                    ..Transform::IDENTITY
                },
            );
            entity
        };
        let a = placed("a", DVec3::new(1.0, 0.0, 0.0), Quat::IDENTITY);
        let quarter = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let b = placed("b", DVec3::new(3.0, 0.0, 0.0), quarter);
        let child = placed("child", DVec3::new(0.0, 0.0, 1.0), Quat::IDENTITY);
        world.set_parent(child, Some(b)).expect("a parent");
        (world, [a, b, child])
    }

    /// A camera 10 m in front of the origin, looking along −Z.
    fn camera() -> Camera {
        Camera {
            eye: DVec3::new(0.0, 0.0, 10.0),
            target: DVec3::ZERO,
            ..Camera::default()
        }
    }

    /// The ray from the camera's eye towards `point`.
    fn ray_to(point: DVec3) -> (DVec3, DVec3) {
        let eye = camera().eye;
        (eye, (point - eye).normalize())
    }

    /// The transform that `command` sets.
    fn set(command: &Command) -> Transform {
        let Command::SetProperty { value, .. } = command else {
            panic!("{command:?}");
        };
        Transform::from_value(value).expect("a transform")
    }

    #[test]
    fn the_handles_stand_at_the_middle_of_the_selection_as_large_wherever_the_camera_is() {
        let (world, [a, b, _]) = world();
        let gizmo = Gizmo::of(&world, &[a, b], Space::World, &camera(), 500.0).expect("placed");
        assert!(near(gizmo.pivot, DVec3::new(2.0, 0.0, 0.0)));
        assert_eq!(gizmo.axes, [DVec3::X, DVec3::Y, DVec3::Z]);
        // Farther, larger, by as much.
        let mut far = camera();
        far.eye.z *= 2.0;
        let farther = Gizmo::of(&world, &[a, b], Space::World, &far, 500.0).expect("placed");
        let ratio = farther.size / gizmo.size;
        let distances = far.eye.distance(gizmo.pivot) / camera().eye.distance(gizmo.pivot);
        assert!((ratio - distances).abs() < 1e-9);
        // In the selection's axes, the last selected's.
        let turned = Gizmo::of(&world, &[a, b], Space::Selection, &camera(), 500.0).expect("");
        assert!(near(turned.axes[0], DVec3::new(0.0, 0.0, -1.0)));
        // Each square drawn across its axis, its sides along the two others.
        for index in 0..3 {
            let square = placed(&turned, Part::Plane(index));
            let normal = square.transform_vector3(DVec3::Y).normalize();
            assert!(near(normal, turned.axes[index]), "{index}: {normal}");
            let side = square.transform_vector3(DVec3::X).normalize();
            let [u, v] = others(index).map(|other| turned.axes[other]);
            assert!(
                near(side.abs(), u.abs()) || near(side.abs(), v.abs()),
                "{index}: {side}"
            );
        }
        assert!(Gizmo::of(&world, &[], Space::World, &camera(), 500.0).is_none());
    }

    #[test]
    fn a_ray_meets_the_part_of_the_handles_that_it_passes_through() {
        let (world, [a, _, _]) = world();
        let gizmo = Gizmo::of(&world, &[a], Space::World, &camera(), 500.0).expect("placed");
        let along = |axis: DVec3, share: f64| gizmo.pivot + axis * gizmo.size * share;
        assert_eq!(
            gizmo.hit(Tool::Move, ray_to(along(DVec3::X, 0.6))),
            Some(Part::Axis(0))
        );
        assert_eq!(
            gizmo.hit(Tool::Move, ray_to(along(DVec3::Y, 0.6))),
            Some(Part::Axis(1))
        );
        // The square of the plane across Z, between X and Y.
        let square = along(DVec3::X + DVec3::Y, PLANE_AT + PLANE_SIDE / 2.0);
        assert_eq!(gizmo.hit(Tool::Move, ray_to(square)), Some(Part::Plane(2)));
        // The ring about Z, seen face on, on its edge between X and Y, where no other ring is.
        let between = (DVec3::X + DVec3::Y).normalize();
        assert_eq!(
            gizmo.hit(Tool::Turn, ray_to(along(between, 1.0))),
            Some(Part::Ring(2))
        );
        assert_eq!(
            gizmo.hit(Tool::Scale, ray_to(gizmo.pivot)),
            Some(Part::Center)
        );
        // Far from every part.
        assert_eq!(
            gizmo.hit(Tool::Move, ray_to(along(DVec3::NEG_X, 2.0))),
            None
        );
    }

    #[test]
    fn a_move_along_an_axis_keeps_to_it_snapped_to_the_world_s_grid() {
        let (world, [a, b, child]) = world();
        let selection = [a, child];
        let gizmo = Gizmo::of(&world, &selection, Space::World, &camera(), 500.0).expect("");
        let start = gizmo.pivot + DVec3::X * gizmo.size * 0.5;
        let drag = Drag::start(
            &world,
            &selection,
            (Tool::Move, Part::Axis(0), gizmo),
            ray_to(start),
        )
        .expect("started");
        assert_eq!(drag.label(), "Move");
        // Dragged 0.6 m along X.
        let to = start + DVec3::new(0.6, 0.0, 0.0);
        let commands = drag.commands(ray_to(to), false);
        let moved = set(&commands[0]);
        assert!(
            near(moved.translation, DVec3::new(1.6, 0.0, 0.0)),
            "{moved:?}"
        );
        // The child moves 0.6 m along the world's X too: along its parent's Z, turned a quarter.
        let child_moved = set(&commands[1]);
        let world_child = DVec3::new(4.0, 0.0, 0.0);
        let expected = (world_child + DVec3::new(0.6, 0.0, 0.0)) - DVec3::new(3.0, 0.0, 0.0);
        let local = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
            .as_dquat()
            .inverse()
            * expected;
        assert!(near(child_moved.translation, local), "{child_moved:?}");
        // A parent and its child selected: the parent alone moves, which carries its child.
        let both = Drag::start(
            &world,
            &[b, child],
            (Tool::Move, Part::Axis(0), gizmo),
            ray_to(start),
        )
        .expect("started");
        assert_eq!(both.commands(ray_to(to), false).len(), 1);
        // A ray along the axis reaches no point of it: nothing to send.
        assert!(drag.commands((camera().eye, DVec3::X), false).is_empty());
        // Snapped, the pivot lands on the grid of a quarter metre: 2.5 + 0.6 lands on 3.0.
        let snapped = drag.commands(ray_to(to), true);
        let moved = set(&snapped[0]);
        assert!((moved.translation.x - 1.5).abs() < 1e-6, "{moved:?}");
    }

    /// Snapping keeps to the axis or the plane moved: in the world's axes, the pivot lands on the
    /// world's grid along them, and keeps its place across them; in the selection's, it moves by
    /// steps from where it started.
    #[test]
    fn snapping_keeps_to_the_axis_or_the_plane_moved() {
        let gizmo = Gizmo {
            pivot: DVec3::new(0.1, 1.1, 0.1),
            rotation: DQuat::IDENTITY,
            axes: [DVec3::X, DVec3::Y, DVec3::Z],
            size: 1.0,
        };
        // In the plane across Y, onto the grid along X and Z, at the height where it stood.
        let moved = snapped(&gizmo, Part::Plane(1), DVec3::new(0.3, 0.0, 0.2));
        assert!(near(moved, DVec3::new(0.4, 0.0, 0.15)), "{moved}");
        let moved = snapped(&gizmo, Part::Axis(1), DVec3::new(0.0, 0.3, 0.0));
        assert!(near(moved, DVec3::new(0.0, 0.4, 0.0)), "{moved}");
        let rotation = DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2);
        let turned = Gizmo {
            rotation,
            axes: [DVec3::X, DVec3::Y, DVec3::Z].map(|axis| rotation * axis),
            ..gizmo
        };
        let moved = snapped(&turned, Part::Axis(0), turned.axes[0] * 0.3);
        assert!(near(moved, turned.axes[0] * 0.25), "{moved}");
    }

    /// A point of the world, in the space of a parent turned and scaled, is where the parent puts
    /// it back.
    #[test]
    fn a_point_is_carried_into_a_turned_and_scaled_parent() {
        let parent = GlobalTransform {
            translation: DVec3::new(1.0, 2.0, 3.0),
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            scale: 2.0,
        };
        let local = DVec3::new(1.0, 0.5, -2.0);
        let world = parent.translation + parent.rotation.as_dquat() * local * 2.0;
        assert!(near(into_parent(&parent, world), local));
    }

    #[test]
    fn a_turn_about_an_axis_turns_about_the_pivot_snapped_to_fifteen_degrees() {
        let (world, [a, _, _]) = world();
        let mut gizmo = Gizmo::of(&world, &[a], Space::World, &camera(), 500.0).expect("placed");
        // About a pivot beside the entity, at the origin.
        gizmo.pivot = DVec3::ZERO;
        let at = |angle: f64| DVec3::new(angle.cos(), angle.sin(), 0.0) * gizmo.size;
        let drag = Drag::start(
            &world,
            &[a],
            (Tool::Turn, Part::Ring(2), gizmo),
            ray_to(at(0.0)),
        )
        .expect("started");
        let turned = set(&drag.commands(ray_to(at(0.5)), false)[0]);
        assert!(near(
            turned.translation,
            DVec3::new(0.5_f64.cos(), 0.5_f64.sin(), 0.0)
        ));
        let snapped = set(&drag.commands(ray_to(at(0.3)), true)[0]);
        let expected = Quat::from_rotation_z(std::f32::consts::PI / 12.0);
        assert!(snapped.rotation.abs_diff_eq(expected, 1e-5), "{snapped:?}");
    }

    #[test]
    fn a_scale_along_an_axis_stretches_and_from_the_middle_scales_whole() {
        let (world, [a, _, _]) = world();
        let gizmo = Gizmo::of(&world, &[a], Space::World, &camera(), 500.0).expect("placed");
        let along = |share: f64| gizmo.pivot + DVec3::Y * gizmo.size * share;
        let drag = Drag::start(
            &world,
            &[a],
            (Tool::Scale, Part::Axis(1), gizmo),
            ray_to(along(0.5)),
        )
        .expect("started");
        let stretched = set(&drag.commands(ray_to(along(1.0)), false)[0]);
        assert!(
            stretched
                .stretch
                .abs_diff_eq(Vec3::new(1.0, 2.0, 1.0), 1e-5)
        );
        assert_eq!(stretched.scale, 1.0);
        let side = |share: f64| gizmo.pivot + DVec3::X * gizmo.size * share;
        let drag = Drag::start(
            &world,
            &[a],
            (Tool::Scale, Part::Center, gizmo),
            ray_to(side(0.1)),
        )
        .expect("started");
        // Moved away by a fifth of the handles' size: a fifth larger, whatever the press's
        // distance from the pivot.
        let scaled = set(&drag.commands(ray_to(side(0.32)), true)[0]);
        assert!((scaled.scale - 1.2).abs() < 1e-4, "{scaled:?}");
        assert!(scaled.stretch.abs_diff_eq(Vec3::ONE, 1e-6));
        let near = Drag::start(
            &world,
            &[a],
            (Tool::Scale, Part::Center, gizmo),
            ray_to(side(0.001)),
        )
        .expect("started");
        let barely = set(&near.commands(ray_to(side(0.05)), false)[0]);
        assert!((barely.scale - 1.049).abs() < 1e-3, "{barely:?}");
    }
}
