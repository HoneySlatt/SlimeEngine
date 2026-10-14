//! What a surface reflects, computed on the CPU independently of the scene shader: glTF's
//! metallic-roughness BRDF (glTF 2.0 specification, Appendix B), in f64. Its visibility term is
//! written from Heitz's height-correlated masking-shadowing, G2 = 1 / (1 + Λ(l) + Λ(v)) with
//! Λ = (√(1 + α² tan² θ) − 1) / 2, divided by 4 (n·l)(n·v): the same function as the shader's
//! rearranged form, so that a slip in either shows. Shared by the tests through `#[path]`;
//! `shading.rs` checks it against closed forms.

use std::f64::consts::PI;

use slime_render::Camera;
use slime_render::glam::DVec3;

/// The smoothest surface the shader shades.
const MIN_ROUGHNESS: f64 = 0.045;
/// What a dielectric reflects at normal incidence: glTF's index of refraction, 1.5.
const DIELECTRIC_F0: f64 = 0.04;

/// A material's values once its textures are read.
#[derive(Clone, Copy, Debug)]
pub struct Surface {
    /// Linear, each channel from 0 to 1.
    pub base_color: [f64; 3],
    /// From 0, a dielectric, to 1, a metal.
    pub metallic: f64,
    /// From 0, a mirror, to 1; shaded no smoother than 0.045, as the shader does.
    pub roughness: f64,
}

/// The light that `surface`, of unit normal `normal`, reflects towards `to_camera` from a
/// directional light of illuminance 1 arriving from `to_light` (unit vectors): the BRDF times n·l;
/// nothing where n·l ≤ 0 or n·v ≤ 0, which the tests do not look at.
pub fn reflected(surface: Surface, normal: DVec3, to_light: DVec3, to_camera: DVec3) -> [f64; 3] {
    let (n_dot_l, n_dot_v) = (normal.dot(to_light), normal.dot(to_camera));
    if n_dot_l <= 0.0 || n_dot_v <= 0.0 {
        return [0.0; 3];
    }
    let alpha = surface.roughness.clamp(MIN_ROUGHNESS, 1.0).powi(2);
    let half = (to_light + to_camera).normalize();
    let (n_dot_h, v_dot_h) = (normal.dot(half).max(0.0), to_camera.dot(half).max(0.0));
    let distribution =
        alpha * alpha / (PI * (n_dot_h * n_dot_h * (alpha * alpha - 1.0) + 1.0).powi(2));
    let lambda =
        |cosine: f64| ((1.0 + alpha * alpha * (1.0 / (cosine * cosine) - 1.0)).sqrt() - 1.0) / 2.0;
    let visibility = 1.0 / (1.0 + lambda(n_dot_l) + lambda(n_dot_v)) / (4.0 * n_dot_l * n_dot_v);
    let specular = distribution * visibility;
    let schlick = (1.0 - v_dot_h).powi(5);
    let dielectric_fresnel = DIELECTRIC_F0 + (1.0 - DIELECTRIC_F0) * schlick;
    surface.base_color.map(|base| {
        let dielectric = base / PI * (1.0 - dielectric_fresnel) + specular * dielectric_fresnel;
        let metal = (base + (1.0 - base) * schlick) * specular;
        (dielectric + (metal - dielectric) * surface.metallic) * n_dot_l
    })
}

/// The unit vector from the point of the plane z = `plane_z` seen at the center of pixel `(x, y)`
/// of a `width` × `height` target, towards `camera`'s eye.
pub fn to_camera(camera: &Camera, size: (u32, u32), pixel: (u32, u32), plane_z: f64) -> DVec3 {
    (camera.eye.as_dvec3() - seen_point(camera, size, pixel, plane_z)).normalize()
}

/// The point of the plane z = `plane_z` seen at the center of pixel `(x, y)` of a `width` ×
/// `height` target, in world space.
pub fn seen_point(
    camera: &Camera,
    (width, height): (u32, u32),
    (x, y): (u32, u32),
    plane_z: f64,
) -> DVec3 {
    let inverse = camera
        .view_projection(width as f32 / height as f32)
        .as_dmat4()
        .inverse();
    let ndc_x = (f64::from(x) + 0.5) / f64::from(width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (f64::from(y) + 0.5) / f64::from(height) * 2.0;
    // Two points along the ray: the near plane, at depth 1, and twice as far, at depth 0.5 (depth
    // is reversed, and 0 is infinitely far).
    let near = inverse.project_point3(DVec3::new(ndc_x, ndc_y, 1.0));
    let far = inverse.project_point3(DVec3::new(ndc_x, ndc_y, 0.5));
    near + (far - near) * ((plane_z - near.z) / (far.z - near.z))
}
