//! The camera: where the scene is seen from, and how it is projected.

use glam::{Mat4, Vec3};

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
    /// Where the camera is, in world space.
    pub eye: Vec3,
    /// The point it looks at, which must differ from `eye`.
    pub target: Vec3,
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
            eye: Vec3::new(0.0, 0.0, 5.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov_y: 60f32.to_radians(),
            near: 0.1,
        }
    }
}

impl Camera {
    /// The view matrix: from world space to the camera's space. A `target` at `eye`, or so near it
    /// that the square of their distance rounds to zero in `f32`, or an `up` that is zero,
    /// parallel to the viewing direction or of an extreme length (see [`up`](Self::up)), gives a
    /// matrix that is not finite and draws nothing.
    ///
    /// # Panics
    ///
    /// Inside glam, for those cameras, when glam's assertions are on (see [`Camera`]).
    pub fn view(&self) -> Mat4 {
        // glam asks for an `up` of length one, and asserts it when its assertions are on.
        glam::camera::rh::view::look_at_mat4(self.eye, self.target, self.up.normalize())
    }

    /// The projection matrix for a target `aspect` times as wide as it is high: from the camera's
    /// space to clip space, +Y up, the convention of WebGPU, which wgpu follows on every graphics
    /// API, with depth reversed (ADR-0047): 1 at [`near`](Self::near), falling towards 0 as the
    /// distance d grows, as `near` / d, without reaching it: the far plane is at infinity. A field
    /// of view outside (0, π) or a `near` that is not positive gives a matrix that draws nothing
    /// usable.
    ///
    /// # Panics
    ///
    /// Inside glam, when glam's assertions are on (see [`Camera`]), for a field of view outside
    /// (0, π), a `near` that is not positive, or an `aspect` of zero.
    pub fn projection(&self, aspect: f32) -> Mat4 {
        glam::camera::rh::proj::directx::perspective_infinite_reverse(self.fov_y, aspect, self.near)
    }

    /// The projection times the view: from world space to clip space.
    ///
    /// # Panics
    ///
    /// As [`view`](Self::view) and [`projection`](Self::projection) do.
    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.projection(aspect) * self.view()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Projects `point` and divides by w: normalized device coordinates.
    fn project(camera: &Camera, point: Vec3) -> Vec3 {
        let clip = camera.view_projection(1.0) * point.extend(1.0);
        clip.truncate() / clip.w
    }

    #[test]
    fn the_target_is_at_the_center_and_depth_falls_from_1_at_near_towards_0_at_infinity() {
        let camera = Camera {
            eye: Vec3::new(0.0, 0.0, 5.0),
            near: 1.0,
            ..Camera::default()
        };
        let center = project(&camera, camera.target);
        assert!(center.x.abs() < 1e-6 && center.y.abs() < 1e-6, "{center}");
        let depth = |distance: f32| project(&camera, Vec3::new(0.0, 0.0, 5.0 - distance)).z;
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
        let clip = camera.view_projection(1.0) * Vec3::NEG_Z.extend(0.0);
        assert!(clip.z == 0.0 && clip.w > 0.0, "{clip}");
    }

    #[test]
    fn the_right_of_the_camera_is_on_the_right_of_the_image() {
        let camera = Camera::default();
        let right = project(&camera, Vec3::new(1.0, 0.0, 0.0));
        let up = project(&camera, Vec3::new(0.0, 1.0, 0.0));
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
}
