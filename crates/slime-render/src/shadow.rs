//! Shadows of the first directional light of a scene that casts shadows: the shadow map's size and
//! depth bias, and the projection that fits the map to the instances (ADR-0038).

use glam::camera::rh::{proj::directx::orthographic, view::look_to_mat4};
use glam::{Mat3, Mat4, Vec3};
use slime_gfx::DepthBias;

/// The width and the height of the shadow map, in texels: 16 MiB of 32-bit depths.
pub(crate) const SHADOW_MAP_SIZE: u32 = 2048;

/// How far the shadow pass pushes away each depth it stores, towards 0 (depth is reversed,
/// ADR-0047), so that a lit surface compared with its own depths is not found behind them (shadow
/// acne). The filter compares a point with texels up to two texels away along each axis of the
/// map: on a surface sloping diagonally to the axes, their depths differ from the point's by up to
/// four times the steeper of its two slopes, the one that the slope-scaled part multiplies (on
/// Direct3D 12, and on lavapipe and RADV as measured).
/// The constant part, 32 units of the depth's last bit, covers what rounding leaves where the
/// slope is zero, on a surface facing the light: the shadow pass and the scene pass compute the
/// same depth along two paths. Its unit shrinks with the depth: the surfaces farthest from the
/// light, the ground under what casts shadows, have the smallest depths, down to about 1/66 with
/// the padding of [`light_view_projection`], whose last bit is up to 64 times finer than near 1.
/// There, 8 suffice on lavapipe and RADV, where 4 sufficed before depth was reversed (ADR-0038).
pub(crate) const SHADOW_BIAS: DepthBias = DepthBias {
    constant: -32,
    slope_scale: -4.0,
};

/// The matrix from world space to the clip space of the shadow map of a directional light
/// travelling along `direction`, not zero: an orthographic projection along the light, whose
/// x and y run from −1 to 1 across the smallest rectangle, perpendicular to the light, that holds
/// every box of `boxes`, and whose depth, reversed as the camera's (ADR-0047), runs from 1 to 0
/// along the light, from a little before the box nearest to it to a little after the farthest. Each box is a transform from its own space
/// to world space, its center and half its size, in its own space: an instance and the box around
/// its mesh.
///
/// The depth range is padded by 1/64 of the largest extent on each side, so that rounding does not
/// clip the instances nearest to the light or farthest from it, and every extent is at least twice
/// that pad, so that a scene flat along one axis keeps a projection. A box at no finite place (an
/// instance thrown to infinity) is left out, so that the others keep their shadows. `None` when
/// no box is left, or when every box is a point: nothing casts a shadow.
pub(crate) fn light_view_projection(
    direction: Vec3,
    boxes: impl IntoIterator<Item = (Mat4, Vec3, Vec3)>,
) -> Option<Mat4> {
    let direction = direction.normalize();
    // Any up direction perpendicular to the light: the map's texels turn with it, nothing else.
    let view = look_to_mat4(
        Vec3::ZERO,
        direction,
        direction.any_orthogonal_vector().normalize(),
    );
    let (mut min, mut max) = (Vec3::INFINITY, Vec3::NEG_INFINITY);
    for (transform, center, half_size) in boxes {
        if !transform.is_finite() {
            continue;
        }
        // The box of a box turned and scaled into the light's space: its center moves with the
        // transform, and each of its half sizes gathers the absolute values of the transform's
        // rows (Arvo's method), exactly the box around its eight corners.
        let to_view = view * transform;
        let center = to_view.transform_point3(center);
        let half_size = Mat3::from_mat4(to_view).abs() * half_size;
        // A finite transform can still carry a box beyond the largest float.
        if !(center.is_finite() && half_size.is_finite()) {
            continue;
        }
        min = min.min(center - half_size);
        max = max.max(center + half_size);
    }
    let pad = (max - min).max_element() / 64.0;
    // No box gives −∞, and boxes that are only points give 0.
    if !pad.is_finite() || pad <= 0.0 {
        return None;
    }
    let center = (min + max) / 2.0;
    let half_size = ((max - min) / 2.0).max(Vec3::splat(pad)) + Vec3::new(0.0, 0.0, pad);
    // The light looks down its −Z axis: the nearest point has the largest z. Its depth is 1, the
    // farthest point's 0: glam's `near` and `far`, swapped.
    let projection = orthographic(
        center.x - half_size.x,
        center.x + half_size.x,
        center.y - half_size.y,
        center.y + half_size.y,
        -(center.z - half_size.z),
        -(center.z + half_size.z),
    );
    Some(projection * view)
}

#[cfg(test)]
mod tests {
    use glam::{EulerRot, Quat, Vec4};

    use super::*;

    /// The eight corners of a box.
    fn corners((transform, center, half_size): (Mat4, Vec3, Vec3)) -> Vec<Vec3> {
        (0..8)
            .map(|corner| {
                let sign = Vec3::new(
                    if corner & 1 == 0 { -1.0 } else { 1.0 },
                    if corner & 2 == 0 { -1.0 } else { 1.0 },
                    if corner & 4 == 0 { -1.0 } else { 1.0 },
                );
                transform.transform_point3(center + sign * half_size)
            })
            .collect()
    }

    /// Where `point` lands in the shadow map's clip space.
    fn projected(matrix: Mat4, point: Vec3) -> Vec3 {
        let clip = matrix * point.extend(1.0);
        assert_eq!(clip.w, 1.0, "orthographic");
        clip.truncate()
    }

    #[test]
    fn every_box_lies_inside_the_map_which_touches_the_outermost_ones() {
        let rotation = Quat::from_euler(EulerRot::XYZ, 0.3, -1.1, 2.0);
        let boxes = [
            // A floor 6 m wide, a cube turned and scaled on it, and a sphere far above.
            (Mat4::IDENTITY, Vec3::ZERO, Vec3::new(3.0, 0.0, 3.0)),
            (
                Mat4::from_scale_rotation_translation(Vec3::new(1.0, 2.0, 0.5), rotation, Vec3::Y),
                Vec3::new(0.1, 0.0, -0.2),
                Vec3::splat(0.5),
            ),
            (
                Mat4::from_translation(Vec3::new(1.0, 30.0, -2.0)),
                Vec3::ZERO,
                Vec3::splat(0.45),
            ),
        ];
        for direction in [
            Vec3::new(-0.5, -1.0, -0.6),
            Vec3::NEG_Y,
            Vec3::Y,
            Vec3::X,
            Vec3::new(0.0, -1e-30, 1.0),
        ] {
            let matrix = light_view_projection(direction, boxes).expect("boxes with a volume");
            assert!(matrix.is_finite(), "{direction}: {matrix}");
            let points: Vec<Vec3> = boxes
                .iter()
                .flat_map(|&b| corners(b))
                .map(|corner| projected(matrix, corner))
                .collect();
            let (min, max) = points.iter().fold(
                (Vec3::INFINITY, Vec3::NEG_INFINITY),
                |(min, max), &point| (min.min(point), max.max(point)),
            );
            // Tight across the light, padded along it.
            assert!(
                min.truncate().abs_diff_eq(-glam::Vec2::ONE, 1e-5)
                    && max.truncate().abs_diff_eq(glam::Vec2::ONE, 1e-5),
                "{direction}: from {min} to {max}"
            );
            assert!(
                min.z > 0.01 && max.z < 0.99,
                "{direction}: depths from {min} to {max}"
            );
            // Depth falls along the light: it is reversed.
            let near = projected(matrix, Vec3::ZERO);
            let far = projected(matrix, direction.normalize());
            assert!(far.z < near.z, "{direction}: {near} then {far}");
        }
    }

    #[test]
    fn a_scene_flat_along_an_axis_still_has_a_finite_projection() {
        // A floor alone, the light straight down onto it, then grazing along it.
        let floor = (Mat4::IDENTITY, Vec3::ZERO, Vec3::new(3.0, 0.0, 3.0));
        for direction in [Vec3::NEG_Y, Vec3::X, Vec3::new(1.0, 0.0, 1.0)] {
            let matrix = light_view_projection(direction, [floor]).expect("a floor has an area");
            assert!(matrix.is_finite(), "{direction}: {matrix}");
            for corner in corners(floor) {
                let point = projected(matrix, corner);
                assert!(
                    point.x.abs() <= 1.0 + 1e-6
                        && point.y.abs() <= 1.0 + 1e-6
                        && point.z > 0.0
                        && point.z < 1.0,
                    "{direction}: {corner} at {point}"
                );
            }
        }
    }

    #[test]
    fn a_box_at_no_finite_place_is_left_out_and_the_others_keep_their_map() {
        let unit = (Mat4::IDENTITY, Vec3::ZERO, Vec3::ONE);
        let alone = light_view_projection(Vec3::NEG_Y, [unit]);
        for far in [
            Mat4::from_translation(Vec3::new(f32::INFINITY, 0.0, 0.0)),
            Mat4::from_translation(Vec3::new(0.0, f32::NAN, 0.0)),
            Mat4::from_scale(Vec3::splat(f32::INFINITY)),
        ] {
            assert_eq!(
                light_view_projection(Vec3::NEG_Y, [unit, (far, Vec3::ZERO, Vec3::ONE)]),
                alone,
                "{far:?}"
            );
        }
        let infinite = Mat4::from_translation(Vec3::splat(f32::INFINITY));
        assert_eq!(
            light_view_projection(Vec3::NEG_Y, [(infinite, Vec3::ZERO, Vec3::ONE)]),
            None,
            "no finite box is left"
        );
    }

    #[test]
    fn nothing_or_only_points_give_no_projection() {
        let none: [(Mat4, Vec3, Vec3); 0] = [];
        assert_eq!(light_view_projection(Vec3::NEG_Y, none), None);
        let point = Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, Vec4::W);
        assert_eq!(
            light_view_projection(Vec3::NEG_Y, [(point, Vec3::ONE, Vec3::ONE)]),
            None
        );
    }
}
