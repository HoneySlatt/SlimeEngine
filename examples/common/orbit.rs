//! The camera of the examples, placed around a point by two angles and a distance.

use slime_render::Camera;
use slime_render::glam::Vec3;

/// A camera at `distance` from `target`, looking at it, turned by `yaw` around the vertical axis
/// (0 looks from +Z) and raised by `pitch` above it (0 is level with it), both in radians, for a
/// scene that lies within `radius` of `target`: its near and far planes enclose that sphere,
/// whatever the scene's size.
pub fn camera_at(target: Vec3, radius: f32, distance: f32, yaw: f32, pitch: f32) -> Camera {
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let (sin_pitch, cos_pitch) = pitch.sin_cos();
    let direction = Vec3::new(cos_pitch * sin_yaw, sin_pitch, cos_pitch * cos_yaw);
    // With a margin for rounding. Inside the sphere, the near plane stays a thousandth of the far
    // one, which the precision of the depth buffer allows.
    let far = (distance + radius) * 1.01;
    let near = ((distance - radius) * 0.99).max(far / 1000.0);
    Camera {
        eye: target + direction * distance,
        target,
        near,
        far,
        ..Camera::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_whole_scene_lies_between_the_near_and_far_planes() {
        for radius in [0.01, 4.4, 500.0] {
            let target = Vec3::new(1.0, -2.0, 3.0) * radius;
            for distance in [0.2 * radius, 2.2 * radius, 15.0 * radius] {
                let camera = camera_at(target, radius, distance, 0.6, 0.45);
                let toward_eye = (camera.eye - target).normalize();
                // The center of the sphere that holds the scene, and its points farthest from
                // the camera and nearest to it, when the camera is outside it.
                let mut points = vec![target, target - toward_eye * radius];
                if distance > radius {
                    points.push(target + toward_eye * radius);
                }
                for point in points {
                    let clip = camera.view_projection(1.0) * point.extend(1.0);
                    let depth = clip.z / clip.w;
                    assert!(
                        (0.0..=1.0).contains(&depth),
                        "radius {radius}, distance {distance}: {point} at depth {depth}"
                    );
                }
            }
        }
    }
}
