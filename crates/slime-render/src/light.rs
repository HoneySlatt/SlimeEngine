//! Lights: what lights the surfaces of a scene.

use glam::Vec3;

/// A light that comes from one direction everywhere, like the sun, together with an ambient
/// light that reaches every surface from all sides.
///
/// Surfaces reflect the directional light with glTF's metallic-roughness model (ADR-0035): a
/// white matte surface facing the light and the camera reflects 0.97 / π of its `color`, less as
/// the light grazes it, and smooth surfaces add highlights. Every surface reflects the ambient
/// light times its base color and its occlusion, whatever its metallic and roughness. Colors are
/// linear, not negative, and may exceed 1; [`Scene`](crate::Scene) bounds the light that reaches
/// a pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectionalLight {
    /// The direction the light travels in, in world space: down and to the side for a sun. Its
    /// length does not matter, but it must not be zero: the renderer would light the scene from
    /// a direction that is not a number, or [`Renderer::render`](crate::Renderer::render) panic
    /// inside glam when glam's assertions are on (see [`Camera`](crate::Camera)).
    pub direction: Vec3,
    /// The illuminance the light gives a surface facing it, per channel: what the light that
    /// surfaces reflect is proportional to. Not negative.
    pub color: Vec3,
    /// The radiance of the ambient light, the same from every direction. Not negative.
    pub ambient: Vec3,
}

impl Default for DirectionalLight {
    /// A white light of π from above and in front of the default camera, under which a white
    /// matte surface facing it reflects about 0.97, and a little ambient light.
    fn default() -> Self {
        Self {
            direction: Vec3::new(-0.5, -1.0, -0.4),
            color: Vec3::splat(std::f32::consts::PI),
            ambient: Vec3::splat(0.12),
        }
    }
}
