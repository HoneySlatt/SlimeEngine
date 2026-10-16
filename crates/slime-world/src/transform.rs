//! Transforms: where an entity is relative to its parent, and in the world.

use glam::{DAffine3, DMat3, DQuat, DVec3, Quat, Vec3};

use crate::Component;

/// How far apart the largest and the smallest of a matrix's three scales may be, relative to the
/// largest, for the matrix to count as scaling uniformly: glTF files exported by Blender give
/// uniform scales that differ in their last digits, such as 1.0007095 and 1.0007096.
const UNIFORM: f64 = 1e-5;

/// Where an entity is relative to its parent, or to the world if it has none
/// (`docs/adr/0079-the-engines-own-world.md`).
///
/// - `translation`, in metres, is `f64`, so that a position 100 km from the origin moves by steps
///   of picometres, where an `f32` would move by steps of 8 mm.
/// - `rotation` turns the entity, its children with it.
/// - `scale`, uniform, scales the entity and its children. A negative one reflects them through
///   the entity's origin, and zero collapses them onto it.
/// - `stretch` scales the entity's own meshes and shapes along its own axes, before its rotation
///   turns them, and its children do not inherit it: a wall or a box longer than it is tall. A
///   negative component mirrors them.
///
/// Children inherit translation, rotation and a uniform scale only, so that a child turning under
/// a stretched parent is never sheared, and where an entity is in the world always splits into a
/// translation, a rotation and a scale ([`GlobalTransform`]). The world computes that again when a
/// transform changes ([`World::global_transform`](crate::World::global_transform)).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// Where the entity's origin is, in its parent's space, in metres.
    pub translation: DVec3,
    /// How the entity is turned, in its parent's space.
    pub rotation: Quat,
    /// How much larger the entity and its children are: 1 for their size.
    pub scale: f32,
    /// How much its own meshes and shapes are stretched along its own axes: `Vec3::ONE` for none.
    pub stretch: Vec3,
}

impl Transform {
    /// No translation, rotation, scale or stretch.
    pub const IDENTITY: Self = Self {
        translation: DVec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: 1.0,
        stretch: Vec3::ONE,
    };

    /// A translation alone.
    pub const fn from_translation(translation: DVec3) -> Self {
        Self {
            translation,
            ..Self::IDENTITY
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Component for Transform {}

/// Where an entity is in the world: its [`Transform`]'s translation, rotation and scale composed
/// with its ancestors', without its stretch, which is its own and apart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalTransform {
    /// Where the entity's origin is in the world, in metres.
    pub translation: DVec3,
    /// How the entity is turned in the world, a unit quaternion.
    pub rotation: Quat,
    /// How much larger the entity is than its meshes' own size, before its stretch.
    pub scale: f32,
}

impl GlobalTransform {
    /// At the origin, not turned, not scaled: where an entity without a parent starts from.
    pub const IDENTITY: Self = Self {
        translation: DVec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: 1.0,
    };

    /// Where a child whose transform is `local` is, when this is where its parent is. The
    /// translations compose in `f64`, the rotations and scales in `f32`.
    pub fn mul_transform(&self, local: &Transform) -> Self {
        Self {
            translation: self.translation
                + self.rotation.as_dquat() * (local.translation * f64::from(self.scale)),
            rotation: (self.rotation * local.rotation).normalize(),
            scale: self.scale * local.scale,
        }
    }

    /// The matrix that places the entity's meshes in the world: this transform, after the entity's
    /// own `stretch`.
    pub fn to_affine(&self, stretch: Vec3) -> DAffine3 {
        DAffine3::from_scale_rotation_translation(
            stretch.as_dvec3() * f64::from(self.scale),
            self.rotation.as_dquat(),
            self.translation,
        )
    }
}

impl Default for GlobalTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// The transform of an imported node whose matrix, relative to its parent's place, is `matrix`,
/// by ADR-0079's rule, and whether its children must be placed apart from it: a scale uniform
/// within [`UNIFORM`] becomes the uniform scale; otherwise the stretch, which the node's children
/// do not inherit, so that a node with children scaled unevenly or mirrored is then flattened, and
/// its children are placed from their own matrices in the world. A shear in the matrix is dropped
/// ([`decompose`]).
pub(crate) fn split(matrix: DAffine3, has_children: bool) -> (Transform, bool) {
    let (scale, rotation, translation) = decompose(matrix);
    let largest = scale.abs().max_element();
    let uniform = scale.min_element() > 0.0 && largest - scale.min_element() <= UNIFORM * largest;
    let transform = Transform {
        translation,
        rotation: rotation.as_quat(),
        scale: 1.0,
        stretch: Vec3::ONE,
    };
    if uniform {
        let scale = scale.element_sum() / 3.0;
        (
            Transform {
                scale: scale as f32,
                ..transform
            },
            false,
        )
    } else {
        (
            Transform {
                stretch: scale.as_vec3(),
                ..transform
            },
            has_children,
        )
    }
}

/// A matrix's scale, rotation and translation, a mirror carried by the scale's `x`, as glam's
/// `DAffine3::to_scale_rotation_translation` gives them, but for any matrix: glam's assertions
/// refuse one that flattens an axis, and a shear makes its rotation a quaternion that is not a
/// unit one. Here the rotation's axes follow the matrix's axes that are not flattened, in order,
/// each made square to those before it, and the flattened ones are completed from them.
fn decompose(matrix: DAffine3) -> (DVec3, DQuat, DVec3) {
    let columns = [
        matrix.matrix3.x_axis,
        matrix.matrix3.y_axis,
        matrix.matrix3.z_axis,
    ];
    let mut scale = DVec3::from_array(columns.map(DVec3::length));
    if matrix.matrix3.determinant() < 0.0 {
        scale.x = -scale.x;
    }
    let mut axes: [Option<DVec3>; 3] = [None; 3];
    for index in 0..3 {
        // A flattened axis divides zero by zero, which `try_normalize` refuses.
        let mut axis = columns[index] / scale[index];
        for earlier in axes.iter().flatten() {
            axis -= axis.dot(*earlier) * *earlier;
        }
        axes[index] = axis.try_normalize();
    }
    let rotation = match axes {
        [Some(x), Some(y), _] => DQuat::from_mat3(&DMat3::from_cols(x, y, x.cross(y))),
        [Some(x), None, Some(z)] => DQuat::from_mat3(&DMat3::from_cols(x, z.cross(x), z)),
        [None, Some(y), Some(z)] => DQuat::from_mat3(&DMat3::from_cols(y.cross(z), y, z)),
        [Some(x), None, None] => DQuat::from_rotation_arc(DVec3::X, x),
        [None, Some(y), None] => DQuat::from_rotation_arc(DVec3::Y, y),
        [None, None, Some(z)] => DQuat::from_rotation_arc(DVec3::Z, z),
        [None, None, None] => DQuat::IDENTITY,
    };
    (scale, rotation, matrix.translation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DMat4, EulerRot};

    fn close(a: DAffine3, b: DAffine3) -> bool {
        a.to_cols_array()
            .iter()
            .zip(b.to_cols_array())
            .all(|(x, y)| (x - y).abs() < 1e-6)
    }

    #[test]
    fn a_child_is_placed_by_its_parent_s_translation_rotation_and_scale_but_not_its_stretch() {
        let parent = GlobalTransform {
            translation: DVec3::new(10.0, 0.0, 0.0),
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            scale: 2.0,
        };
        let child = Transform {
            translation: DVec3::new(1.0, 0.0, 0.0),
            stretch: Vec3::new(5.0, 1.0, 1.0),
            ..Transform::IDENTITY
        };
        let placed = parent.mul_transform(&child);
        // Turned a quarter about +Y, +X becomes −Z; scaled twice.
        assert!((placed.translation - DVec3::new(10.0, 0.0, -2.0)).length() < 1e-6);
        assert_eq!(placed.scale, 2.0);
        // The child's own stretch places its meshes, not its children.
        let meshes = placed.to_affine(child.stretch);
        let corner = meshes.transform_point3(DVec3::new(1.0, 0.0, 0.0));
        assert!((corner - DVec3::new(10.0, 0.0, -12.0)).length() < 1e-5);
    }

    #[test]
    fn a_child_1_mm_from_a_parent_10_000_km_away_stays_1_mm_from_it() {
        let parent = GlobalTransform {
            translation: DVec3::new(1.0e7, 0.0, -1.0e7),
            ..GlobalTransform::IDENTITY
        };
        let child = parent.mul_transform(&Transform::from_translation(DVec3::new(0.001, 0.0, 0.0)));
        assert!((child.translation.x - parent.translation.x - 0.001).abs() < 1e-9);
    }

    #[test]
    fn uniform_scales_split_into_the_scale_and_uneven_ones_into_the_stretch() {
        let rotation = DQuat::from_euler(EulerRot::YXZ, 0.3, 0.2, 0.1);
        let matrix = |scale: DVec3| {
            DAffine3::from_scale_rotation_translation(scale, rotation, DVec3::new(1.0, 2.0, 3.0))
        };
        // Blender's uniform scales differ in their last digits.
        let (transform, flattened) =
            split(matrix(DVec3::new(1.0007095, 1.0007096, 1.0007095)), true);
        assert!((transform.scale - 1.0007095).abs() < 1e-6);
        assert_eq!(transform.stretch, Vec3::ONE);
        assert!(!flattened);
        let box_scale = DVec3::new(2.0, 0.5, 3.0);
        let (leaf, flattened) = split(matrix(box_scale), false);
        assert_eq!(leaf.scale, 1.0);
        assert!((leaf.stretch.as_dvec3() - box_scale).length() < 1e-6);
        assert!(!flattened, "a leaf keeps its stretch for itself");
        let (_, flattened) = split(matrix(box_scale), true);
        assert!(
            flattened,
            "a node with children cannot pass an uneven scale on"
        );
        // The transform places the node where its matrix did.
        let placed = GlobalTransform::IDENTITY
            .mul_transform(&leaf)
            .to_affine(leaf.stretch);
        assert!(close(placed, matrix(box_scale)));
    }

    #[test]
    fn a_mirroring_scale_is_a_stretch_that_places_the_node_where_its_matrix_did() {
        let matrix = DAffine3::from_mat4(DMat4::from_scale(DVec3::new(-1.0, 1.0, 1.0)));
        let (transform, _) = split(matrix, false);
        let placed = GlobalTransform::IDENTITY
            .mul_transform(&transform)
            .to_affine(transform.stretch);
        assert!(close(placed, matrix));
        assert!(placed.matrix3.determinant() < 0.0);
    }

    #[test]
    fn a_matrix_that_flattens_an_axis_keeps_its_rotation() {
        let rotation = DQuat::from_euler(EulerRot::YXZ, 0.7, 0.2, 0.1);
        for scale in [
            DVec3::new(1.0, 0.0, 2.0),
            DVec3::new(0.0, 1.0, 2.0),
            DVec3::new(1.0, 2.0, 0.0),
            DVec3::new(-1.0, 0.0, 2.0),
            DVec3::new(0.0, 0.0, 3.0),
            DVec3::ZERO,
        ] {
            let matrix = DAffine3::from_scale_rotation_translation(scale, rotation, DVec3::X);
            let (transform, _) = split(matrix, false);
            assert!(transform.rotation.is_normalized(), "{scale}");
            let placed = GlobalTransform::IDENTITY
                .mul_transform(&transform)
                .to_affine(transform.stretch);
            assert!(
                close(placed, matrix),
                "{scale}: {placed:?} against {matrix:?}"
            );
        }
    }

    #[test]
    fn a_sheared_matrix_keeps_its_first_axis_and_a_unit_rotation() {
        // A child turned under a parent stretched three to one.
        let matrix = DAffine3::from_scale(DVec3::new(3.0, 1.0, 1.0))
            * DAffine3::from_quat(DQuat::from_rotation_y(0.7));
        let (transform, _) = split(matrix, false);
        assert!(transform.rotation.is_normalized());
        let placed = GlobalTransform::IDENTITY
            .mul_transform(&transform)
            .to_affine(transform.stretch);
        let first_axis = |matrix: DAffine3| matrix.matrix3.x_axis.normalize();
        assert!((first_axis(placed) - first_axis(matrix)).length() < 1e-6);
    }
}
