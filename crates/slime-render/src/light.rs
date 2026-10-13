//! Lights: what lights the surfaces of a scene.

use glam::Vec3;

/// A light that comes from one direction everywhere, like the sun, together with an ambient
/// light that reaches every surface from all sides.
///
/// A surface is lit in proportion to the cosine of the angle between its normal and the
/// direction to the light (Lambert's law), plus the ambient light; both are multiplied by the
/// material's base color. Colors are linear, not negative, and may exceed 1 to light more
/// strongly; [`Scene`](crate::Scene) bounds the light that reaches a pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectionalLight {
    /// The direction the light travels in, in world space: down and to the side for a sun. Its
    /// length does not matter, but it must not be zero: the renderer would light the scene from
    /// a direction that is not a number, or [`Renderer::render`](crate::Renderer::render) panic
    /// inside glam when glam's assertions are on (see [`Camera`](crate::Camera)).
    pub direction: Vec3,
    /// The color and strength of the directional light: not negative.
    pub color: Vec3,
    /// The color and strength of the ambient light: not negative.
    pub ambient: Vec3,
}

impl Default for DirectionalLight {
    /// A white light from above and in front of the default camera, and a little ambient light.
    fn default() -> Self {
        Self {
            direction: Vec3::new(-0.5, -1.0, -0.4),
            color: Vec3::ONE,
            ambient: Vec3::splat(0.12),
        }
    }
}
