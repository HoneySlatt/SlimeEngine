//! The camera: where the scene is seen from, and how it is projected.

use glam::{DMat4, DVec3, Mat4, Vec3, Vec4};

/// A perspective camera, looking from [`eye`](Self::eye) at [`target`](Self::target).
///
/// World space is right-handed with +Y up, as in glTF; the camera looks down its own −Z axis.
///
/// A camera that [`view`](Self::view) or [`projection`](Self::projection) says draws nothing can
/// panic inside glam instead when glam's assertions are on: its `glam-assert` feature, or
/// `debug-glam-assert` in a debug build. A game that enables either on the glam it shares with
/// the engine, the version [`crate::glam`] re-exports, enables it in the engine too.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Where the camera is, in world space, in metres: `f64`, as positions in the world are
    /// (ADR-0079).
    pub eye: DVec3,
    /// The point it looks at, which must differ from `eye`.
    pub target: DVec3,
    /// Which way is up, usually [`Vec3::Y`]. Its length does not matter, short of one so small or
    /// so large that its square rounds to zero or overflows in `f32`, but it must not be zero or
    /// parallel to the viewing direction.
    pub up: Vec3,
    /// The vertical field of view, in radians.
    pub fov_y: f32,
    /// The distance of the nearest visible point, positive. Nothing is too far to be seen: depth
    /// is reversed and the far plane is at infinity (ADR-0047), so a larger `near` only hides what
    /// is nearer, without making the depth of far surfaces finer.
    pub near: f32,
}

impl Default for Camera {
    /// Five units in front of the origin, looking at it, with a field of view of 60°, seeing from
    /// 0.1 away.
    fn default() -> Self {
        Self {
            eye: DVec3::new(0.0, 0.0, 5.0),
            target: DVec3::ZERO,
            up: Vec3::Y,
            fov_y: 60f32.to_radians(),
            near: 0.1,
        }
    }
}

impl Camera {
    /// The view matrix: from world space to the camera's space, in `f64`, as world positions are.
    /// A `target` at `eye`, or so near it that the square of their distance rounds to zero, or an
    /// `up` that is zero, parallel to the viewing direction or of an extreme length (see
    /// [`up`](Self::up)), gives a matrix that is not finite and draws nothing.
    ///
    /// # Panics
    ///
    /// Inside glam, for those cameras, when glam's assertions are on (see [`Camera`]).
    pub fn view(&self) -> DMat4 {
        // glam asks for an `up` of length one, and asserts it when its assertions are on.
        glam::dcamera::rh::view::look_at_mat4(self.eye, self.target, self.up.as_dvec3().normalize())
    }

    /// The view matrix from world space moved by −[`eye`](Self::eye), the space in which the
    /// renderer draws (ADR-0048): the rotation of [`view`](Self::view) alone. The direction from
    /// the eye to the target is computed in `f64`, so that a camera far from the origin keeps it.
    pub(crate) fn view_from_eye(&self) -> Mat4 {
        let direction = (self.target - self.eye).as_vec3();
        glam::camera::rh::view::look_at_mat4(Vec3::ZERO, direction, self.up.normalize())
    }

    /// The projection matrix for a target `aspect` times as wide as it is high: from the camera's
    /// space to clip space, +Y up, the convention of WebGPU, which wgpu follows on every graphics
    /// API, with depth reversed (ADR-0047): 1 at [`near`](Self::near), falling towards 0 as the
    /// distance d grows, as `near` / d, without reaching it: the far plane is at infinity, and
    /// 32-bit float depths keep about the same relative precision at every distance. A field of
    /// view outside (0, π) or a `near` that is not positive gives a matrix that draws nothing
    /// usable.
    ///
    /// # Panics
    ///
    /// Inside glam, when glam's assertions are on (see [`Camera`]), for a field of view outside
    /// (0, π), a `near` that is not positive, or an `aspect` of zero.
    pub fn projection(&self, aspect: f32) -> Mat4 {
        glam::camera::rh::proj::directx::perspective_infinite_reverse(self.fov_y, aspect, self.near)
    }

    /// The projection times the view: from world space to clip space, in `f64` as
    /// [`view`](Self::view) is.
    ///
    /// # Panics
    ///
    /// As [`view`](Self::view) and [`projection`](Self::projection) do.
    pub fn view_projection(&self, aspect: f32) -> DMat4 {
        self.projection(aspect).as_dmat4() * self.view()
    }
}

/// What a camera sees, for frustum culling: the space between the four planes through the eye and
/// the sides of the image, beyond the near plane, in the frame's space (ADR-0048). It has no far
/// plane: nothing is too far to be seen (ADR-0047).
pub(crate) struct Frustum {
    /// Each plane as (n, d), n pointing inside: a point p is on its inner side when n · p + d is
    /// not negative. Left, right, bottom, top, near.
    planes: [Vec4; 5],
}

impl Frustum {
    /// The frustum of `view_projection`, the matrix from the frame's space to clip space, whose
    /// rows give its planes (Gribb and Hartmann): a point is seen where −w ≤ x ≤ w, −w ≤ y ≤ w and
    /// z ≤ w, reversed depth putting the near plane at z = w, and 0 ≤ z holding everywhere.
    pub(crate) fn new(view_projection: Mat4) -> Self {
        let [x, y, z, w] = [0, 1, 2, 3].map(|row| view_projection.row(row));
        Self {
            planes: [w + x, w - x, w + y, w - y, w - z],
        }
    }

    /// Whether the box of an instance may be seen: `transform` takes it from its own space, where
    /// it has its center and half its size along each axis, to the frame's space. A box that some
    /// plane leaves wholly outside is not seen; one that crosses each plane, or that lies inside,
    /// is, even when it lies outside two planes near a corner, which costs a draw and keeps the
    /// test cheap. A box at no finite place is not seen.
    pub(crate) fn holds(&self, (transform, center, half_size): (Mat4, Vec3, Vec3)) -> bool {
        let center = transform.transform_point3(center);
        // The box's half axes in the frame's space: how far it reaches from its center along a
        // plane's normal is the sum of their lengths along it.
        let axes = [
            transform.x_axis.truncate() * half_size.x,
            transform.y_axis.truncate() * half_size.y,
            transform.z_axis.truncate() * half_size.z,
        ];
        self.planes.iter().all(|plane| {
            let normal = plane.truncate();
            let reach: f32 = axes.iter().map(|axis| normal.dot(*axis).abs()).sum();
            // Not a number, for a box at no finite place, compares as false: not seen.
            normal.dot(center) + plane.w >= -reach
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Projects `point` and divides by w: normalized device coordinates.
    fn project(camera: &Camera, point: DVec3) -> DVec3 {
        let clip = camera.view_projection(1.0) * point.extend(1.0);
        clip.truncate() / clip.w
    }

    #[test]
    fn the_target_is_at_the_center_and_depth_falls_from_1_at_near_towards_0_at_infinity() {
        let camera = Camera {
            eye: DVec3::new(0.0, 0.0, 5.0),
            near: 1.0,
            ..Camera::default()
        };
        let center = project(&camera, camera.target);
        assert!(center.x.abs() < 1e-6 && center.y.abs() < 1e-6, "{center}");
        let depth = |distance: f64| project(&camera, DVec3::new(0.0, 0.0, 5.0 - distance)).z;
        assert!((depth(1.0) - 1.0).abs() < 1e-6, "near plane at depth 1");
        assert!((depth(4.0) - 0.25).abs() < 1e-6, "near / distance");
        // Far beyond any far plane the camera had, depth still orders distances: a float keeps
        // its relative precision down to 1e-38.
        let (far, farther) = (depth(1e7), depth(1.1e7));
        assert!(
            0.0 < farther && farther < far && far < 1e-6,
            "{far}, {farther}"
        );
        // A direction, a point at infinity, is at depth 0.
        let clip = camera.view_projection(1.0) * DVec3::NEG_Z.extend(0.0);
        assert!(clip.z == 0.0 && clip.w > 0.0, "{clip}");
    }

    #[test]
    fn the_right_of_the_camera_is_on_the_right_of_the_image() {
        let camera = Camera::default();
        let right = project(&camera, DVec3::new(1.0, 0.0, 0.0));
        let up = project(&camera, DVec3::new(0.0, 1.0, 0.0));
        assert!(right.x > 0.0 && right.y.abs() < 1e-6, "{right}");
        assert!(up.y > 0.0 && up.x.abs() < 1e-6, "{up}");
    }

    /// glam's `look_at_mat4` asks for an `up` of length one and asserts it when glam's assertions
    /// are on; with them off, it ignores the length. This test can therefore fail only with them
    /// on, as in `cargo test -p slime-render --features glam/debug-glam-assert`, which
    /// `scripts/check.sh` runs.
    #[test]
    fn the_length_of_up_does_not_change_the_view() {
        let unit = Camera::default().view();
        for length in [2.0, 1e-3] {
            let camera = Camera {
                up: Vec3::Y * length,
                ..Camera::default()
            };
            assert!(camera.view().abs_diff_eq(unit, 1e-6), "length {length}");
        }
    }

    /// The frustum of the default camera, looking down −Z from 5 m in front of the origin with a
    /// field of view of 60°, on a square image: in the frame's space, the eye is at the origin.
    fn frustum() -> Frustum {
        let camera = Camera::default();
        Frustum::new(camera.projection(1.0) * camera.view_from_eye())
    }

    /// A box of half size `half` at `center`, in the frame's space.
    fn boxed(center: Vec3, half: f32) -> (Mat4, Vec3, Vec3) {
        (
            Mat4::from_translation(center),
            Vec3::ZERO,
            Vec3::splat(half),
        )
    }

    #[test]
    fn a_box_in_view_or_across_its_edges_is_seen_and_one_beyond_them_is_not() {
        let frustum = frustum();
        // 10 m ahead, the image spans tan(30°) × 10 = 5.77 m on each side of the axis.
        for (center, half, seen, case) in [
            (Vec3::new(0.0, 0.0, -10.0), 0.5, true, "ahead"),
            (
                Vec3::new(6.0, 0.0, -10.0),
                0.5,
                true,
                "across the right edge",
            ),
            (
                Vec3::new(7.0, 0.0, -10.0),
                0.5,
                false,
                "beyond the right edge",
            ),
            (
                Vec3::new(0.0, -7.0, -10.0),
                0.5,
                false,
                "beyond the bottom edge",
            ),
            (Vec3::new(0.0, 0.0, 10.0), 0.5, false, "behind the eye"),
            (Vec3::new(0.0, 0.0, 0.0), 0.5, true, "around the eye"),
            (
                Vec3::new(0.0, 0.0, -0.05),
                0.01,
                false,
                "nearer than the near plane",
            ),
            (
                Vec3::new(0.0, 0.0, -1e9),
                1.0,
                true,
                "farther than any far plane",
            ),
        ] {
            assert_eq!(frustum.holds(boxed(center, half)), seen, "{case}");
        }
    }

    #[test]
    fn a_box_turned_and_scaled_reaches_as_far_as_its_corners() {
        let frustum = frustum();
        // A rod 4 m long along X, its center 7.5 m right of the axis 10 m ahead: lying along X it
        // reaches 5.5 m right of the axis, in view; turned along Y, it does not.
        let at = Mat4::from_translation(Vec3::new(7.5, 0.0, -10.0));
        let rod = Mat4::from_scale(Vec3::new(4.0, 0.1, 0.1));
        let along_x = at * rod;
        let along_y = at * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2) * rod;
        let unit = (Vec3::ZERO, Vec3::splat(0.5));
        assert!(frustum.holds((along_x, unit.0, unit.1)), "along X");
        assert!(!frustum.holds((along_y, unit.0, unit.1)), "along Y");
    }

    #[test]
    fn the_frustum_follows_where_the_camera_looks_and_the_image_s_aspect() {
        // Looking along +X from the origin, at an image twice as wide as it is high: 10 m ahead, it
        // spans tan(30°) × 10 = 5.77 m up and down, twice that, 11.5 m, to each side.
        let camera = Camera {
            eye: DVec3::ZERO,
            target: DVec3::X,
            ..Camera::default()
        };
        let frustum = Frustum::new(camera.projection(2.0) * camera.view_from_eye());
        for (center, seen, case) in [
            (Vec3::new(10.0, 0.0, 0.0), true, "ahead"),
            (Vec3::new(-10.0, 0.0, 0.0), false, "behind"),
            (Vec3::new(10.0, 0.0, 11.0), true, "11 m to the side"),
            (Vec3::new(10.0, 0.0, 12.5), false, "12.5 m to the side"),
            (Vec3::new(10.0, 7.0, 0.0), false, "7 m up"),
        ] {
            assert_eq!(frustum.holds(boxed(center, 0.25)), seen, "{case}");
        }
    }

    #[test]
    fn a_box_at_no_finite_place_is_not_seen() {
        let frustum = frustum();
        for value in [f32::NAN, f32::INFINITY] {
            let transform = Mat4::from_translation(Vec3::new(0.0, 0.0, value));
            assert!(
                !frustum.holds((transform, Vec3::ZERO, Vec3::ONE)),
                "{value}"
            );
        }
    }
}
