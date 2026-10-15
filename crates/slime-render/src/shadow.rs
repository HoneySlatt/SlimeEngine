//! Shadows: those of the first directional light of a scene that casts shadows, its cascades, their
//! depth bias, and the projection that fits each to its slice of the view (ADR-0038, ADR-0050,
//! ADR-0064); and those of spot lights, the tiles of their atlas and what each sees
//! (ADR-0058, ADR-0061).

use glam::camera::rh::proj::directx::{orthographic, perspective_infinite_reverse};
use glam::camera::rh::view::look_to_mat4;
use glam::{DMat4, DVec3, DVec4, Mat3, Mat4, Vec3};
use slime_gfx::DepthBias;

use crate::camera::Camera;
use crate::light::{MAX_LIGHTS, SpotLight, unit};

/// How far the shadow pass pushes away each depth it stores, towards 0 (depth is reversed,
/// ADR-0047), so that a lit surface compared with its own depths is not found behind them (shadow
/// acne): 32 units of the depth's last bit, which cover what rounding leaves, as the shadow pass
/// and the scene pass compute a depth along two paths. Its unit shrinks with the depth: the
/// surfaces farthest from the light, the ground under what casts shadows, have the smallest depths,
/// down to about 1/66 with the padding of [`CascadeView::view_projection`], whose last bit is up to
/// 64 times finer than near 1; there, 4 suffice on lavapipe and RADV. Nothing scales with the
/// surface's slope: the scene shader looks a surface up off itself along its normal instead
/// (`NORMAL_OFFSET_TEXELS` in `shader.wgsl`, ADR-0050), by what the filter's reach across a sloping
/// or curved surface needs.
pub(crate) const SHADOW_BIAS: DepthBias = DepthBias {
    constant: -32,
    slope_scale: 0.0,
};

/// How many cascades the shadows of the directional light are split into, each a tile of the
/// cascades' atlas covering a slice of what the camera sees (ADR-0064).
pub(crate) const CASCADES: usize = 4;

/// The width and the height of each cascade's tile, in texels.
pub(crate) const CASCADE_SIZE: u32 = 2048;

/// The width and the height of the cascades' atlas, 2 × 2 tiles: 64 MiB of 32-bit depths.
pub(crate) const CASCADE_ATLAS_SIZE: u32 = 2 * CASCADE_SIZE;

/// A tile of a shadow atlas, the cascades' or the spot lights': its top left texel and its side,
/// in texels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ShadowTile {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) size: u32,
}

/// The tile of cascade `index` in the cascades' atlas, row by row from its top left corner.
pub(crate) fn cascade_tile(index: usize) -> ShadowTile {
    let index = u32::try_from(index).expect("below CASCADES");
    ShadowTile {
        x: index % 2 * CASCADE_SIZE,
        y: index / 2 * CASCADE_SIZE,
        size: CASCADE_SIZE,
    }
}

/// The slice of what `camera` sees, along its axis, that cascade `index` covers when its light
/// casts shadows up to `distance`: each cascade twice as deep as the one before, so that the four
/// end at 1, 3, 7 and 15 fifteenths of the distance; the first starts at the camera's near plane
/// (ADR-0064). A slice that would end before it starts ends at its start.
pub(crate) fn cascade_slice(camera: &Camera, distance: f32, index: usize) -> (f32, f32) {
    let end = |index: usize| {
        let shares = (1u32 << (index + 1)) - 1;
        distance * shares as f32 / ((1u32 << CASCADES) - 1) as f32
    };
    let near = if index == 0 {
        camera.near
    } else {
        end(index - 1).max(camera.near)
    };
    (near, end(index).max(near))
}

/// What a cascade of the directional light sees: an orthographic view along the light of the
/// square, across it, around the sphere that holds the cascade's slice of what the camera sees
/// ([`view_sphere`]), moved across the light to a corner of the texels of a grid fixed in the world
/// (ADR-0050).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CascadeView {
    /// From the frame's space (world space moved by −the camera's eye, ADR-0048) to the light's:
    /// x and y across it, z against it.
    view: Mat4,
    /// Half the side of the square, in metres.
    half_side: f32,
    /// The width of the cascade's texels, in metres.
    pub(crate) texel: f32,
}

impl CascadeView {
    /// The view of the cascade covering the slice of what `camera` sees from `near` to `far` along
    /// its axis, for an output `aspect` times as wide as it is high, of a directional light
    /// travelling along `direction`, not zero. The square's size depends on the camera's field of
    /// view, `aspect` and the slice only, not on where the camera looks; its center is moved across
    /// the light to a corner of a texel of a grid fixed in the world, computed in `f64`, so that as
    /// the camera turns and moves, each texel keeps covering the same ground and shadows' edges do
    /// not shimmer. `None` for a camera that sees nothing (see [`Camera`]).
    pub(crate) fn new(
        direction: Vec3,
        camera: &Camera,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Option<Self> {
        let (center, radius) = view_sphere(camera, aspect, near, far);
        if !(center.is_finite() && radius.is_finite() && radius > 0.0) {
            return None;
        }
        let texel = 2.0 * radius / f64::from(CASCADE_SIZE);
        // The light's axes as glam's views have them: x and y across the light, z against it.
        let forward = direction.as_dvec3().normalize();
        let x = forward.cross(forward.any_orthogonal_vector()).normalize();
        let y = x.cross(forward);
        // Across the light, the center moves to the nearest multiple of a texel from the world's
        // origin, computed in f64 so that the grid stays exact far from it.
        let eye = camera.eye.as_dvec3();
        let snapped =
            |axis: DVec3| ((eye + center).dot(axis) / texel).round() * texel - eye.dot(axis);
        let center = x * snapped(x) + y * snapped(y) + forward * center.dot(forward);
        let view = DMat4::from_cols(
            DVec4::new(x.x, y.x, -forward.x, 0.0),
            DVec4::new(x.y, y.y, -forward.y, 0.0),
            DVec4::new(x.z, y.z, -forward.z, 0.0),
            DVec4::new(-x.dot(center), -y.dot(center), forward.dot(center), 1.0),
        )
        .as_mat4();
        Some(Self {
            view,
            half_side: radius as f32,
            texel: texel as f32,
        })
    }

    /// The box of `(transform, center, half_size)` in the light's space, if it is somewhere and
    /// not flattened to a point: its center and half its size. The transform takes the box from
    /// its own space to the frame's space, its center and half its size in its own space.
    fn in_light_space(
        &self,
        (transform, center, half_size): (Mat4, Vec3, Vec3),
    ) -> Option<(Vec3, Vec3)> {
        if !transform.is_finite() {
            return None;
        }
        // The box of a box turned and scaled into the light's space: its center moves with the
        // transform, and each of its half sizes gathers the absolute values of the transform's
        // rows (Arvo's method), exactly the box around its eight corners.
        let to_view = self.view * transform;
        let center = to_view.transform_point3(center);
        let half_size = Mat3::from_mat4(to_view).abs() * half_size;
        // A finite transform can still carry a box beyond the largest float.
        let placed = center.is_finite() && half_size.is_finite() && half_size != Vec3::ZERO;
        placed.then_some((center, half_size))
    }

    /// Whether the box of an instance that casts shadows, as [`Self::in_light_space`] takes it,
    /// reaches across the cascade's square: only such a box can shadow a point of it.
    pub(crate) fn reaches(&self, boxed: (Mat4, Vec3, Vec3)) -> bool {
        self.in_light_space(boxed)
            .is_some_and(|(center, half_size)| {
                center.x.abs() - half_size.x <= self.half_side
                    && center.y.abs() - half_size.y <= self.half_side
            })
    }

    /// The matrix from the frame's space to the cascade's clip space: x and y from −1 to 1 across
    /// its square, and depth, reversed as the camera's (ADR-0047), from 1 to 0 along the light,
    /// from a little before the nearest of `boxes` that reach across the square to a little after
    /// the farthest; padded by 1/64 of the larger of the square's side and the range itself on each
    /// side, so that rounding does not clip them. `None` when no box reaches across it.
    pub(crate) fn view_projection(
        &self,
        boxes: impl IntoIterator<Item = (Mat4, Vec3, Vec3)>,
    ) -> Option<Mat4> {
        // The light looks down its −Z axis: the nearest point has the largest z.
        let (mut nearest, mut farthest) = (f32::NEG_INFINITY, f32::INFINITY);
        for boxed in boxes {
            if !self.reaches(boxed) {
                continue;
            }
            let (center, half_size) = self.in_light_space(boxed).expect("it reaches");
            nearest = nearest.max(center.z + half_size.z);
            farthest = farthest.min(center.z - half_size.z);
        }
        if farthest > nearest {
            return None;
        }
        let half_side = self.half_side;
        let pad = (2.0 * half_side).max(nearest - farthest) / 64.0;
        // The nearest point at depth 1, the farthest at 0: glam's `near` and `far`, swapped.
        let projection = orthographic(
            -half_side,
            half_side,
            -half_side,
            half_side,
            -(farthest - pad),
            -(nearest + pad),
        );
        Some(projection * self.view)
    }
}

/// The sphere around what `camera` sees, for an output `aspect` times as wide as it is high, from
/// `near` to `far` along its axis, or to `near` when `far` is nearer: its center, relative to the
/// eye, and its radius, in f64. It is the smallest sphere centred on the view's axis that holds the
/// corners of both planes: its radius depends on the field of view, the aspect and the two
/// distances, not on where the camera looks.
pub(crate) fn view_sphere(camera: &Camera, aspect: f32, near: f32, far: f32) -> (DVec3, f64) {
    let near = f64::from(near);
    let far = f64::from(far).max(near);
    let tan_y = (f64::from(camera.fov_y) / 2.0).tan();
    // The squared tangent of the angle from the axis to a corner.
    let corner = tan_y * tan_y * (1.0 + f64::from(aspect) * f64::from(aspect));
    // The point of the axis as far from a corner of the near plane as from one of the far plane,
    // or the far plane's center when that point lies beyond it: a wide view, whose far corners
    // alone set the sphere.
    let along = ((far + near) * (corner + 1.0) / 2.0).min(far);
    let radius = (far * far * corner + (far - along) * (far - along)).sqrt();
    let forward = (camera.target.as_dvec3() - camera.eye.as_dvec3()).normalize();
    (forward * along, radius)
}

/// The width and the height of the spot lights' shadow atlas, in texels: 64 MiB of 32-bit depths
/// (ADR-0058).
pub(crate) const SPOT_ATLAS_SIZE: u32 = 4096;

/// How far from a spot light its tile starts seeing, in metres: what lies nearer casts no shadow,
/// so that the lamp a light sits in does not hide it (ADR-0061).
pub(crate) const SPOT_NEAR: f32 = 0.05;

/// The widest a spot light's tile sees, in radians from its axis: 80°. A cone of up to π / 2 would
/// need a tile infinitely wide; beyond this angle, its light casts no shadow (ADR-0061).
pub(crate) const SPOT_WIDEST: f32 = 1.396_263_4;

/// How many texels a tile sees beyond its light's cone on each side, so that the filter of a point
/// at the cone's edge, looked up off its surface, still reads the tile's own texels: the filter
/// reads texels up to three from the point (ADR-0063), which the normal offset moves by up to 4.5
/// (`NORMAL_OFFSET_TEXELS` in `shader.wgsl`, ADR-0061).
const TILE_MARGIN_TEXELS: u32 = 8;

/// The tiles of the atlas for `count` spot lights that cast shadows, in the scene's order: 2048 ×
/// 2048 texels each for up to four lights, 1024 × 1024 each from five to [`MAX_LIGHTS`], row by
/// row from the atlas's top left corner (ADR-0058).
///
/// # Panics
///
/// If `count` is above [`MAX_LIGHTS`], the most lights a scene draws.
pub(crate) fn spot_tiles(count: usize) -> impl Iterator<Item = ShadowTile> {
    assert!(count <= MAX_LIGHTS, "{count} spot lights cast shadows");
    let per_row: u32 = if count <= 4 { 2 } else { 4 };
    let size = SPOT_ATLAS_SIZE / per_row;
    (0..u32::try_from(count).expect("at most MAX_LIGHTS")).map(move |index| ShadowTile {
        x: index % per_row * size,
        y: index / per_row * size,
        size,
    })
}

/// What the tile of a spot light sees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SpotView {
    /// From the frame's space (world space moved by −the camera's eye, ADR-0048) to the tile's
    /// clip space: x and y from −1 to 1 across the tile, depth `SPOT_NEAR` / d at a distance d
    /// along the light's axis, reversed and infinite as the camera's (ADR-0047).
    pub(crate) view_projection: Mat4,
    /// Where the light is, in the frame's space.
    pub(crate) position: Vec3,
    /// The light's axis, of length one.
    pub(crate) axis: Vec3,
    /// The tangent of the angle from the axis to the tile's sides.
    pub(crate) tan_half: f32,
}

impl SpotView {
    /// The view of a tile of `size` texels for `light`, in the frame's space of a camera whose eye
    /// is `origin`: a perspective projection from the light along its axis, square, wide enough to
    /// hold its cone, up to [`SPOT_WIDEST`] from the axis, and [`TILE_MARGIN_TEXELS`] more on each
    /// side, from [`SPOT_NEAR`] to infinity. The light's position is taken relative to `origin` in
    /// `f64` (ADR-0048).
    pub(crate) fn new(light: &SpotLight, origin: Vec3, size: u32) -> Self {
        let position = (light.position.as_dvec3() - origin.as_dvec3()).as_vec3();
        let axis = unit(light.direction);
        let cone = light.outer_cone_angle.min(SPOT_WIDEST).tan();
        let tan_half = cone * size as f32 / (size - 2 * TILE_MARGIN_TEXELS) as f32;
        let view = look_to_mat4(position, axis, axis.any_orthonormal_vector());
        let projection = perspective_infinite_reverse(2.0 * tan_half.atan(), 1.0, SPOT_NEAR);
        Self {
            view_projection: projection * view,
            position,
            axis,
            tan_half,
        }
    }

    /// Whether the box of an instance, `transform` from its own space to the frame's space, its
    /// center and half its size in its own space, may cast a shadow in the tile of a light that
    /// reaches `range` metres (`None` for every distance): whether the sphere around the box meets
    /// the cone around the tile's corners, in front of the light and within its range. A box at
    /// no finite place is left out.
    pub(crate) fn reaches(
        &self,
        range: Option<f32>,
        (transform, center, half_size): (Mat4, Vec3, Vec3),
    ) -> bool {
        let center = transform.transform_point3(center);
        let radius = (Mat3::from_mat4(transform).abs() * half_size).length();
        if !(center.is_finite() && radius.is_finite()) {
            return false;
        }
        let to_center = center - self.position;
        let along = to_center.dot(self.axis);
        let across = (to_center.length_squared() - along * along).max(0.0).sqrt();
        // The cone around the square tile passes through its corners.
        let tan_corner = self.tan_half * std::f32::consts::SQRT_2;
        let (sin, cos) = tan_corner.atan().sin_cos();
        // The distance from the sphere's center to the cone's surface, positive outside it.
        let outside = across * cos - along * sin;
        let in_range = range.is_none_or(|range| to_center.length() - radius < range);
        outside < radius && along > -radius && in_range
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use glam::{Quat, Vec4};

    use super::*;

    const ASPECT: f32 = 16.0 / 9.0;

    /// The view of a single cascade covering what `camera` sees up to `distance`, fitted to
    /// `boxes`: what every cascade does over its own slice.
    fn light_view_projection(
        direction: Vec3,
        camera: &Camera,
        aspect: f32,
        distance: f32,
        boxes: impl IntoIterator<Item = (Mat4, Vec3, Vec3)>,
    ) -> Option<Mat4> {
        CascadeView::new(direction, camera, aspect, camera.near, distance)?.view_projection(boxes)
    }

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

    /// A camera at `eye` looking along `forward`.
    fn camera(eye: Vec3, forward: Vec3) -> Camera {
        Camera {
            eye,
            target: eye + forward,
            ..Camera::default()
        }
    }

    /// A box 1 m wide around `center`, given in the frame's space.
    fn cube(center: Vec3) -> (Mat4, Vec3, Vec3) {
        (Mat4::from_translation(center), Vec3::ZERO, Vec3::splat(0.5))
    }

    /// The points of the view of `camera`, up to `distance`, relative to the eye: the corners of
    /// its near and far planes and a few points between.
    fn seen(camera: &Camera, distance: f32) -> Vec<Vec3> {
        let inverse = (camera.projection(ASPECT) * camera.view_from_eye()).inverse();
        let mut points = Vec::new();
        for depth in [1.0, camera.near / distance, camera.near / (distance / 2.0)] {
            for (x, y) in [
                (-1.0, -1.0),
                (1.0, -1.0),
                (1.0, 1.0),
                (-1.0, 1.0),
                (0.0, 0.0),
            ] {
                points.push(inverse.project_point3(Vec3::new(x, y, depth)));
            }
        }
        points
    }

    #[test]
    fn the_map_covers_what_the_camera_sees_up_to_the_distance_whichever_way_it_looks() {
        let direction = Vec3::new(-0.5, -1.0, -0.6);
        let mut sides = Vec::new();
        for forward in [
            Vec3::NEG_Z,
            Vec3::X,
            Vec3::new(0.3, -0.8, 0.2),
            Vec3::new(-1.0, 0.2, 3.0),
        ] {
            let camera = camera(Vec3::new(3.0, 2.0, -1.0), forward);
            let distance = 40.0;
            // Something to cast across the map, below the camera.
            let floor = (
                Mat4::IDENTITY,
                Vec3::new(0.0, -2.0, 0.0),
                Vec3::new(100.0, 0.0, 100.0),
            );
            let matrix = light_view_projection(direction, &camera, ASPECT, distance, [floor])
                .expect("the floor casts across the map");
            for point in seen(&camera, distance) {
                let at = projected(matrix, point);
                assert!(
                    at.x.abs() <= 1.0 && at.y.abs() <= 1.0,
                    "{forward}: {point} at {at}"
                );
            }
            // Its size, the length of a unit across the light in the map's clip space.
            sides.push(matrix.x_axis.truncate().length());
        }
        for side in &sides {
            assert!((side - sides[0]).abs() <= 1e-6 * sides[0], "{sides:?}");
        }
    }

    #[test]
    fn as_the_camera_moves_the_texels_stay_on_a_grid_fixed_in_the_world() {
        let direction = Vec3::new(0.2, -1.0, 0.4);
        let forward = direction.as_dvec3().normalize();
        let axes = {
            let x = forward.cross(forward.any_orthogonal_vector()).normalize();
            [x, x.cross(forward)]
        };
        let floor = (Mat4::IDENTITY, Vec3::ZERO, Vec3::new(1e5, 0.0, 1e5));
        let offset = Vec3::new(1.25, -2.0, -2.5);
        for eye in [
            Vec3::new(0.0, 2.0, 5.0),
            Vec3::new(0.013, 2.0, 5.004),
            Vec3::new(-7.31, 2.5, 3.9),
            Vec3::new(10_000.0, 2.0, -10_000.0),
        ] {
            let camera = camera(eye, Vec3::new(0.0, -0.3, -1.0));
            let (_, radius) = view_sphere(&camera, ASPECT, camera.near, 30.0);
            let texel = 2.0 * radius / f64::from(CASCADE_SIZE);
            let relative = (Mat4::from_translation(-eye) * floor.0, floor.1, floor.2);
            let matrix = light_view_projection(direction, &camera, ASPECT, 30.0, [relative])
                .expect("the floor casts across the map");
            // A point near the camera lies across a texel as its place in the world, measured along
            // the light's axes from the world's origin, says: texels' edges lie on a grid fixed in
            // the world, wherever the camera is.
            let at = projected(matrix, offset);
            let point = eye.as_dvec3() + offset.as_dvec3();
            for (index, axis) in axes.iter().enumerate() {
                let texels = (f64::from(at[index]) + 1.0) / 2.0 * f64::from(CASCADE_SIZE);
                let expected = point.dot(*axis) / texel;
                let apart = (texels - expected).rem_euclid(1.0);
                assert!(
                    apart.min(1.0 - apart) < 2e-3,
                    "{eye}, axis {index}: {texels} texels, {expected} expected"
                );
            }
        }
    }

    #[test]
    fn only_what_reaches_across_the_map_sets_its_depths() {
        let camera = camera(Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, -0.5, -1.0));
        let direction = Vec3::NEG_Y;
        let near = cube(Vec3::new(0.0, -1.0, -5.0));
        let alone = light_view_projection(direction, &camera, ASPECT, 20.0, [near])
            .expect("the cube casts across the map");
        // A tower 2 km away, beside the map, changes nothing.
        let beside = (
            Mat4::from_translation(Vec3::new(2000.0, 0.0, 0.0)),
            Vec3::ZERO,
            Vec3::new(5.0, 300.0, 5.0),
        );
        assert_eq!(
            light_view_projection(direction, &camera, ASPECT, 20.0, [near, beside]),
            Some(alone)
        );
        // A cloud above the map, between it and the light, casts on it: it sets the near end.
        let above = cube(Vec3::new(1.0, 500.0, -5.0));
        let matrix = light_view_projection(direction, &camera, ASPECT, 20.0, [near, above])
            .expect("both cast across the map");
        for corner in corners(near).into_iter().chain(corners(above)) {
            let depth = projected(matrix, corner).z;
            assert!(depth > 0.0 && depth < 1.0, "{corner} at depth {depth}");
        }
        // Depth falls along the light: it is reversed.
        assert!(projected(matrix, Vec3::ZERO).z < projected(matrix, Vec3::Y).z);
        // Nothing across the map: no shadow.
        assert_eq!(
            light_view_projection(direction, &camera, ASPECT, 20.0, [beside]),
            None
        );
    }

    #[test]
    fn a_scene_flat_along_the_light_still_has_a_finite_projection() {
        // A floor alone, the light straight down onto it, then grazing along it.
        let camera = camera(Vec3::new(0.5, 1.0, 2.0), Vec3::new(0.0, -0.4, -1.0));
        let floor = (Mat4::IDENTITY, Vec3::ZERO, Vec3::new(3.0, 0.0, 3.0));
        for direction in [Vec3::NEG_Y, Vec3::X, Vec3::new(1.0, 0.0, 1.0)] {
            let matrix = light_view_projection(direction, &camera, ASPECT, 10.0, [floor])
                .expect("a floor has an area");
            assert!(matrix.is_finite(), "{direction}: {matrix}");
            for corner in corners(floor) {
                let depth = projected(matrix, corner).z;
                assert!(
                    depth > 0.0 && depth < 1.0,
                    "{direction}: {corner} at depth {depth}"
                );
            }
        }
    }

    #[test]
    fn a_box_at_no_finite_place_or_flattened_to_a_point_is_left_out() {
        let camera = camera(Vec3::new(0.0, 1.0, 3.0), Vec3::NEG_Z);
        let unit = cube(Vec3::ZERO);
        let alone = light_view_projection(Vec3::NEG_Y, &camera, ASPECT, 10.0, [unit]);
        let point = Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, Vec4::W);
        for left_out in [
            Mat4::from_translation(Vec3::new(f32::INFINITY, 0.0, 0.0)),
            Mat4::from_translation(Vec3::new(0.0, f32::NAN, 0.0)),
            Mat4::from_scale(Vec3::splat(f32::INFINITY)),
            point,
        ] {
            assert_eq!(
                light_view_projection(
                    Vec3::NEG_Y,
                    &camera,
                    ASPECT,
                    10.0,
                    [unit, (left_out, Vec3::ZERO, Vec3::ONE)]
                ),
                alone,
                "{left_out:?}"
            );
        }
        let none: [(Mat4, Vec3, Vec3); 0] = [];
        assert_eq!(
            light_view_projection(Vec3::NEG_Y, &camera, ASPECT, 10.0, none),
            None
        );
    }

    #[test]
    fn the_sphere_holds_the_corners_of_the_near_and_far_planes() {
        for (fov_degrees, aspect) in [(60.0f32, 1.0), (60.0, ASPECT), (120.0, 3.0), (10.0, 0.5)] {
            let camera = Camera {
                fov_y: fov_degrees.to_radians(),
                ..camera(Vec3::ZERO, Vec3::new(0.2, 0.1, -1.0))
            };
            // The whole view up to 50 m, then slices of it, as the cascades have them.
            for (near, far) in [(camera.near, 50.0), (6.0, 14.0), (14.0, 30.0)] {
                let (center, radius) = view_sphere(&camera, aspect, near, far);
                let inverse = (camera.projection(aspect) * camera.view_from_eye())
                    .as_dmat4()
                    .inverse();
                let mut largest = 0.0f64;
                // Depth is the camera's near plane over the distance (ADR-0047).
                for distance in [near, far] {
                    let depth = f64::from(camera.near) / f64::from(distance);
                    for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                        let corner = inverse.project_point3(glam::DVec3::new(x, y, depth));
                        largest = largest.max(corner.distance(center));
                    }
                }
                assert!(
                    largest <= radius * (1.0 + 1e-6) && largest >= radius * (1.0 - 1e-6),
                    "{fov_degrees}°, {aspect}, {near} to {far} m: corners up to {largest} from \
                     the center, radius {radius}"
                );
            }
        }
    }

    #[test]
    fn the_cascades_split_the_distance_each_twice_as_deep_as_the_one_before() {
        let camera = camera(Vec3::ZERO, Vec3::NEG_Z);
        let slices: Vec<(f32, f32)> = (0..CASCADES)
            .map(|index| cascade_slice(&camera, 30.0, index))
            .collect();
        assert_eq!(
            slices,
            [(camera.near, 2.0), (2.0, 6.0), (6.0, 14.0), (14.0, 30.0)]
        );
        // A distance nearer than the camera's near plane leaves slices of no depth there.
        let short: Vec<(f32, f32)> = (0..CASCADES)
            .map(|index| cascade_slice(&camera, 0.5, index))
            .collect();
        assert_eq!(short[0], (camera.near, camera.near));
        assert_eq!(short[3], (7.0 / 30.0, 0.5));
        // The tiles fill the atlas, 2 × 2.
        let tiles: Vec<ShadowTile> = (0..CASCADES).map(cascade_tile).collect();
        let tile = |x, y| ShadowTile { x, y, size: 2048 };
        assert_eq!(
            tiles,
            [tile(0, 0), tile(2048, 0), tile(0, 2048), tile(2048, 2048)]
        );
    }

    #[test]
    fn each_cascade_covers_its_slice_and_keeps_the_boxes_that_reach_across_it() {
        let direction = Vec3::new(-0.5, -1.0, -0.6);
        let floor = (
            Mat4::IDENTITY,
            Vec3::new(0.0, -2.0, 0.0),
            Vec3::new(100.0, 0.0, 100.0),
        );
        for forward in [Vec3::NEG_Z, Vec3::new(0.3, -0.8, 0.2)] {
            let camera = camera(Vec3::new(3.0, 2.0, -1.0), forward);
            let mut texels = Vec::new();
            for index in 0..CASCADES {
                let (near, far) = cascade_slice(&camera, 30.0, index);
                let cascade = CascadeView::new(direction, &camera, ASPECT, near, far)
                    .expect("the camera sees something");
                texels.push(cascade.texel);
                let matrix = cascade
                    .view_projection([floor])
                    .expect("the floor casts across every cascade");
                let inverse = (camera.projection(ASPECT) * camera.view_from_eye()).inverse();
                for distance in [near, far] {
                    for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                        let point = inverse.project_point3(Vec3::new(x, y, camera.near / distance));
                        let at = projected(matrix, point);
                        assert!(
                            at.x.abs() <= 1.0 && at.y.abs() <= 1.0,
                            "{forward}, cascade {index}: {point} at {at}"
                        );
                    }
                }
                // A cube under the slice's middle reaches across it; one 1 km beside it does not.
                let middle = forward.normalize() * (near + far) / 2.0;
                assert!(cascade.reaches(cube(middle)));
                assert!(!cascade.reaches(cube(middle + Vec3::new(1000.0, 0.0, 0.0))));
            }
            // Each cascade's texels are finer than the next one's.
            assert!(
                texels.windows(2).all(|pair| pair[0] < pair[1]),
                "{texels:?}"
            );
        }
    }

    #[test]
    fn spot_tiles_split_the_atlas_by_how_many_lights_cast_shadows() {
        assert_eq!(spot_tiles(0).count(), 0);
        let tile = |x, y, size| ShadowTile { x, y, size };
        assert_eq!(spot_tiles(1).collect::<Vec<_>>(), [tile(0, 0, 2048)]);
        assert_eq!(
            spot_tiles(4).collect::<Vec<_>>(),
            [
                tile(0, 0, 2048),
                tile(2048, 0, 2048),
                tile(0, 2048, 2048),
                tile(2048, 2048, 2048)
            ]
        );
        assert_eq!(
            spot_tiles(5).collect::<Vec<_>>(),
            [
                tile(0, 0, 1024),
                tile(1024, 0, 1024),
                tile(2048, 0, 1024),
                tile(3072, 0, 1024),
                tile(0, 1024, 1024)
            ]
        );
        // Sixteen tiles fill the atlas without overlapping.
        let tiles: Vec<ShadowTile> = spot_tiles(MAX_LIGHTS).collect();
        let mut covered = [false; 16];
        for tile in &tiles {
            assert_eq!(tile.size, 1024);
            assert!(tile.x + tile.size <= SPOT_ATLAS_SIZE && tile.y + tile.size <= SPOT_ATLAS_SIZE);
            let cell = (tile.y / 1024 * 4 + tile.x / 1024) as usize;
            assert!(!covered[cell], "{tile:?} overlaps another");
            covered[cell] = true;
        }
    }

    #[test]
    #[should_panic(expected = "17 spot lights cast shadows")]
    fn more_spot_tiles_than_lights_drawn_are_refused() {
        let _ = spot_tiles(MAX_LIGHTS + 1);
    }

    fn spot(position: Vec3, direction: Vec3, outer: f32, range: Option<f32>) -> SpotLight {
        SpotLight {
            position,
            direction,
            outer_cone_angle: outer,
            range,
            ..SpotLight::default()
        }
    }

    #[test]
    fn a_spot_tile_holds_its_cone_with_a_margin_for_the_filter() {
        // A light 10 km from the world's origin, near the camera: the view is relative to it.
        let origin = Vec3::new(10_000.0, 0.0, 0.0);
        let direction = Vec3::new(1.0, -2.0, 0.5);
        for (outer, size) in [(0.42, 1024), (0.35, 2048), (1.2, 1024)] {
            let light = spot(origin + Vec3::new(1.0, 3.0, 2.0), direction, outer, None);
            let view = SpotView::new(&light, origin, size);
            let axis = direction.normalize();
            assert!(view.position.abs_diff_eq(Vec3::new(1.0, 3.0, 2.0), 1e-6));
            // Points of the cone's surface, in eight directions around the axis, near and far.
            let across = axis.any_orthogonal_vector().normalize();
            for turn in 0..8 {
                let around = Quat::from_axis_angle(axis, turn as f32 * std::f32::consts::FRAC_PI_4);
                let edge = Quat::from_axis_angle(around * across, outer) * axis;
                for distance in [0.5f32, 4.0, 30.0] {
                    let along = distance / edge.dot(axis);
                    let clip = view.view_projection * (view.position + edge * along).extend(1.0);
                    let projected = clip.truncate() / clip.w;
                    // The cone's edge is a circle the margin inside the tile's sides.
                    let radius = projected.truncate().length();
                    let inside = 1.0 - 2.0 * TILE_MARGIN_TEXELS as f32 / size as f32;
                    assert!((radius - inside).abs() < 1e-4, "{outer}, {turn}: {radius}");
                    assert!((projected.z - SPOT_NEAR / distance).abs() < 1e-6);
                }
            }
        }
        // A cone of π / 2 is seen up to 80° from its axis.
        let wide = SpotView::new(&spot(origin, direction, FRAC_PI_2, None), origin, 1024);
        let cone = SPOT_WIDEST.tan();
        assert!((wide.tan_half / cone - 1024.0 / 1008.0).abs() < 1e-5);
    }

    #[test]
    fn a_spot_tile_keeps_the_boxes_inside_its_cone_and_range() {
        let origin = Vec3::ZERO;
        let light = spot(Vec3::new(0.0, 5.0, 0.0), Vec3::NEG_Y, 0.4, Some(6.0));
        let view = SpotView::new(&light, origin, 1024);
        let cube = |at: Vec3, side: f32| {
            (
                Mat4::from_scale_rotation_translation(Vec3::splat(side), Quat::IDENTITY, at),
                Vec3::ZERO,
                Vec3::splat(0.5),
            )
        };
        let keeps = |boxed| view.reaches(light.range, boxed);
        // Under the light, inside its cone.
        assert!(keeps(cube(Vec3::new(0.5, 1.0, 0.0), 1.0)));
        // Beside the cone, 4 m out at 4 m down: 45° from the axis, beyond the 31° of the tile's
        // corners.
        assert!(!keeps(cube(Vec3::new(4.0, 1.0, 0.0), 1.0)));
        // A large box beside it reaches into it.
        assert!(keeps(cube(Vec3::new(4.0, 1.0, 0.0), 6.0)));
        // Behind the light.
        assert!(!keeps(cube(Vec3::new(0.0, 7.0, 0.0), 1.0)));
        // Beyond its range, and across it.
        assert!(!keeps(cube(Vec3::new(0.0, -2.0, 0.0), 1.0)));
        assert!(keeps(cube(Vec3::new(0.0, -0.9, 0.0), 1.0)));
        // Without a range, as far as the cone goes.
        assert!(
            SpotView::new(
                &spot(Vec3::new(0.0, 5.0, 0.0), Vec3::NEG_Y, 0.4, None),
                origin,
                1024
            )
            .reaches(None, cube(Vec3::new(0.0, -200.0, 0.0), 1.0))
        );
        // A box thrown to infinity.
        let thrown = (
            Mat4::from_translation(Vec3::INFINITY),
            Vec3::ZERO,
            Vec3::ONE,
        );
        assert!(!keeps(thrown));
    }
}
