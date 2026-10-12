//! The camera of the examples, placed around a point by two angles and a distance.

use slime_render::Camera;
use slime_render::glam::Vec3;

/// A camera at `distance` from `target`, looking at it, turned by `yaw` around the vertical axis
/// (0 looks from +Z) and raised by `pitch` above it (0 is level with it), both in radians.
pub fn camera_at(target: Vec3, distance: f32, yaw: f32, pitch: f32) -> Camera {
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let (sin_pitch, cos_pitch) = pitch.sin_cos();
    let direction = Vec3::new(cos_pitch * sin_yaw, sin_pitch, cos_pitch * cos_yaw);
    Camera {
        eye: target + direction * distance,
        target,
        ..Camera::default()
    }
}
