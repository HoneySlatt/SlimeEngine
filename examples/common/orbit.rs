//! The camera of the examples, placed around a point by two angles and a distance.

use slime_render::Camera;
use slime_render::glam::{DVec3, Vec3};

/// A camera at `distance` from `target`, looking at it, turned by `yaw` around the vertical axis
/// (0 looks from +Z) and raised by `pitch` above it (0 is level with it), both in radians, for a
/// scene that lies within `radius` of `target`. Its near plane is a thousandth of `radius` from the
/// eye: what lies nearer is clipped. Nothing is too far: the far plane is at infinity, and depth
/// is reversed, so that its precision does not depend on the near plane (ADR-0047).
pub fn camera_at(target: DVec3, radius: f32, distance: f32, yaw: f32, pitch: f32) -> Camera {
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let (sin_pitch, cos_pitch) = pitch.sin_cos();
    let direction = Vec3::new(cos_pitch * sin_yaw, sin_pitch, cos_pitch * cos_yaw);
    Camera {
        eye: target + direction.as_dvec3() * f64::from(distance),
        target,
        near: radius / 1000.0,
        ..Camera::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_whole_scene_lies_beyond_the_near_plane() {
        for radius in [0.01, 4.4, 500.0] {
            let target = DVec3::new(1.0, -2.0, 3.0) * f64::from(radius);
            for distance in [0.2 * radius, 2.2 * radius, 15.0 * radius] {
                let camera = camera_at(target, radius, distance, 0.6, 0.45);
                let toward_eye = (camera.eye - target).normalize();
                // The center of the sphere that holds the scene, and its points farthest from
                // the camera and nearest to it, when the camera is outside it.
                let mut points = vec![target, target - toward_eye * f64::from(radius)];
                if distance > radius {
                    points.push(target + toward_eye * f64::from(radius));
                }
                for point in points {
                    let clip = camera.view_projection(1.0) * point.extend(1.0);
                    let depth = clip.z / clip.w;
                    assert!(
                        depth > 0.0 && depth <= 1.0,
                        "radius {radius}, distance {distance}: {point} at depth {depth}"
                    );
                }
            }
        }
    }
}
