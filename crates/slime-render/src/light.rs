//! Lights: what lights the surfaces of a scene, in the units of glTF's `KHR_lights_punctual`
//! (ADR-0037).

use std::f32::consts::FRAC_PI_4;

use glam::Vec3;

use crate::bytes::{push_f32s, push_u32s};

/// The most lights a [`Scene`](crate::Scene) holds:
/// [`Renderer::render`](crate::Renderer::render) panics on more. Every fragment of the scene pass
/// evaluates every light of the scene (ADR-0037).
pub const MAX_LIGHTS: usize = 16;

/// The size of one light's values, as `Light` in `shader.wgsl` lays them out: four 16-byte rows.
pub(crate) const LIGHT_VALUES_SIZE: usize = 64;

/// The kinds of light as the scene shader numbers them (`Light::kind` in `shader.wgsl`).
const DIRECTIONAL: u32 = 0;
const POINT: u32 = 1;
const SPOT: u32 = 2;

/// A light of a scene: one of glTF's punctual lights (`KHR_lights_punctual`), each in its
/// physical unit.
///
/// Surfaces reflect each light with glTF's metallic-roughness model (ADR-0035): the light that
/// reaches a surface facing it, its illuminance, times the BRDF and the cosine of incidence. A
/// white matte surface facing a light and the camera reflects 0.97 / π of the illuminance it
/// receives, in nits (candelas per square metre) for an illuminance in lux. Distances are in
/// metres, as in glTF.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Light {
    /// A light from one direction everywhere, like the sun.
    Directional(DirectionalLight),
    /// A light from a point, in every direction.
    Point(PointLight),
    /// A light from a point, in a cone.
    Spot(SpotLight),
}

/// A light that comes from one direction everywhere, like the sun, without falloff
/// (`KHR_lights_punctual`'s `directional`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectionalLight {
    /// The direction the light travels in, in world space: down and to the side for a sun. Its
    /// length does not matter, but it must not be zero: the renderer would light the scene from
    /// a direction that is not a number, or [`Renderer::render`](crate::Renderer::render) panic
    /// inside glam when glam's assertions are on (see [`Camera`](crate::Camera)).
    pub direction: Vec3,
    /// Linear, not negative: what `illuminance` is multiplied by in each channel. glTF keeps each
    /// channel within 0 and 1, so that `illuminance` alone sets the strength.
    pub color: Vec3,
    /// The illuminance the light gives a surface facing it, in lux (lumens per square metre):
    /// about 100 000 for the sun on a clear day, 400 at sunrise. Finite and not negative.
    pub illuminance: f32,
    /// Whether the light casts shadows. The renderer has one shadow map (ADR-0038): the first
    /// directional light of the scene that casts shadows casts them, and the others light
    /// everything they face, as point and spot lights do. A scene whose directional lights all
    /// set it to `false` draws no shadow, and skips the shadow pass.
    pub casts_shadows: bool,
}

impl Default for DirectionalLight {
    /// A white light of 1 lux, glTF's default, from above and in front of the default camera,
    /// casting shadows.
    fn default() -> Self {
        Self {
            direction: Vec3::new(-0.5, -1.0, -0.4),
            color: Vec3::ONE,
            illuminance: 1.0,
            casts_shadows: true,
        }
    }
}

/// A light that comes from a point, the same in every direction, like a bare bulb
/// (`KHR_lights_punctual`'s `point`).
///
/// A surface at distance d receives `intensity` / d² lux, brought smoothly to 0 at `range`: the
/// falloff that `KHR_lights_punctual` recommends, `intensity` × clamp(1 − (d / range)⁴, 0, 1) /
/// d². A surface nearer than 1 cm receives what it would 1 cm away, so that one through the light
/// is not lit infinitely; within a micrometre of the light, it has no direction to the light and
/// receives nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointLight {
    /// Where the light is, in world space.
    pub position: Vec3,
    /// Linear, not negative: what `intensity` is multiplied by in each channel. glTF keeps each
    /// channel within 0 and 1, so that `intensity` alone sets the strength.
    pub color: Vec3,
    /// The luminous intensity, in candelas (lumens per steradian): the illuminance, in lux, of a
    /// surface facing the light 1 metre away. About 1 for a candle, 64 for a household bulb of
    /// 800 lumens. Finite and not negative.
    pub intensity: f32,
    /// The distance, in metres, at and beyond which the light reaches nothing; `None` for a light
    /// that reaches every distance. Positive and finite.
    pub range: Option<f32>,
}

impl Default for PointLight {
    /// A white light of 1 candela, glTF's default, at the origin, without range.
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            color: Vec3::ONE,
            intensity: 1.0,
            range: None,
        }
    }
}

/// A light that comes from a point, in a cone, like a torch or a stage light
/// (`KHR_lights_punctual`'s `spot`).
///
/// It falls off with distance as a [`PointLight`] does. Within `inner_cone_angle` of its
/// direction it lights fully, beyond `outer_cone_angle` not at all, and in between the light is
/// multiplied by t², where t goes from 1 to 0 linearly with the cosine of the angle
/// (`KHR_lights_punctual`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpotLight {
    /// Where the light is, in world space.
    pub position: Vec3,
    /// The direction the light travels in along the cone's axis, in world space. Its length does
    /// not matter, but it must not be zero, as for [`DirectionalLight::direction`].
    pub direction: Vec3,
    /// As [`PointLight::color`].
    pub color: Vec3,
    /// The luminous intensity inside the inner cone, in candelas. Finite and not negative.
    pub intensity: f32,
    /// As [`PointLight::range`].
    pub range: Option<f32>,
    /// The angle from the axis, in radians, within which the light is full: from 0 to below
    /// `outer_cone_angle`.
    pub inner_cone_angle: f32,
    /// The angle from the axis, in radians, beyond which there is no light: at most π / 2.
    pub outer_cone_angle: f32,
}

impl Default for SpotLight {
    /// A white light of 1 candela at the origin, pointing down −Z, without range, with glTF's
    /// cone: full on its axis only, nothing beyond π / 4.
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            direction: Vec3::NEG_Z,
            color: Vec3::ONE,
            intensity: 1.0,
            range: None,
            inner_cone_angle: 0.0,
            outer_cone_angle: FRAC_PI_4,
        }
    }
}

impl Light {
    /// Appends the values the scene shader reads for this light, laid out as `Light` in
    /// `shader.wgsl`: [`LIGHT_VALUES_SIZE`] bytes. What a kind of light does not use is zero, but
    /// for the cone: a scale of 0 and an offset of 1 leave a point light's light whole.
    pub(crate) fn push_values(&self, bytes: &mut Vec<u8>) {
        let start = bytes.len();
        let (kind, color, intensity, position, range, direction, cone) = match *self {
            Self::Directional(light) => (
                DIRECTIONAL,
                light.color,
                light.illuminance,
                Vec3::ZERO,
                None,
                light.direction.normalize(),
                (0.0, 1.0),
            ),
            Self::Point(light) => (
                POINT,
                light.color,
                light.intensity,
                light.position,
                light.range,
                Vec3::ZERO,
                (0.0, 1.0),
            ),
            Self::Spot(light) => (
                SPOT,
                light.color,
                light.intensity,
                light.position,
                light.range,
                light.direction.normalize(),
                cone_scale_and_offset(light.inner_cone_angle, light.outer_cone_angle),
            ),
        };
        let strength = color * intensity;
        push_f32s(bytes, &strength.to_array());
        push_u32s(bytes, &[kind]);
        // 1 / range², so that the shader's (d / range)⁴ is (d² / range²)², and 0 for no range.
        let inverse_range_squared = range.map_or(0.0, |range| 1.0 / (range * range));
        push_f32s(bytes, &position.to_array());
        push_f32s(bytes, &[inverse_range_squared]);
        push_f32s(bytes, &direction.to_array());
        push_f32s(bytes, &[cone.0, cone.1, 0.0, 0.0, 0.0]);
        debug_assert_eq!(bytes.len() - start, LIGHT_VALUES_SIZE);
    }
}

/// The scale and offset that turn the cosine of the angle from a spot light's axis into its
/// cone's attenuation before it is squared, clamped to 0 to 1: 1 at the inner angle and 0 at the
/// outer one (`KHR_lights_punctual`'s reference code, whose floor of 0.001 keeps a cone whose two
/// angles are equal finite).
fn cone_scale_and_offset(inner: f32, outer: f32) -> (f32, f32) {
    let (cos_inner, cos_outer) = (inner.cos(), outer.cos());
    let scale = 1.0 / (cos_inner - cos_outer).max(0.001);
    (scale, -cos_outer * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(bytes: &[u8]) -> Vec<[u8; 4]> {
        bytes
            .chunks(4)
            .map(|word| word.try_into().unwrap())
            .collect()
    }

    fn float(word: [u8; 4]) -> f32 {
        f32::from_le_bytes(word)
    }

    #[test]
    fn each_kind_of_light_is_packed_as_the_shader_reads_it() {
        let mut bytes = Vec::new();
        Light::Spot(SpotLight {
            position: Vec3::new(1.0, 2.0, 3.0),
            direction: Vec3::new(0.0, -2.0, 0.0),
            color: Vec3::new(1.0, 0.5, 0.25),
            intensity: 8.0,
            range: Some(4.0),
            inner_cone_angle: 0.0,
            outer_cone_angle: std::f32::consts::FRAC_PI_2,
        })
        .push_values(&mut bytes);
        assert_eq!(bytes.len(), LIGHT_VALUES_SIZE);
        let packed = words(&bytes);
        let floats: Vec<f32> = packed.iter().map(|&word| float(word)).collect();
        assert_eq!(
            &floats[..3],
            &[8.0, 4.0, 2.0],
            "the color times the intensity"
        );
        assert_eq!(u32::from_le_bytes(packed[3]), SPOT);
        assert_eq!(&floats[4..8], &[1.0, 2.0, 3.0, 1.0 / 16.0]);
        assert_eq!(
            &floats[8..11],
            &[0.0, -1.0, 0.0],
            "the direction, normalized"
        );
        // cos 0 = 1 and cos π/2 ≈ 0: the cone's attenuation is the cosine itself.
        assert!((floats[11] - 1.0).abs() < 1e-6 && floats[12].abs() < 1e-6);
        assert_eq!(&floats[13..], &[0.0; 3]);

        let mut bytes = Vec::new();
        Light::Point(PointLight::default()).push_values(&mut bytes);
        let packed = words(&bytes);
        assert_eq!(u32::from_le_bytes(packed[3]), POINT);
        assert_eq!(float(packed[7]), 0.0, "no range");
        assert_eq!(
            (float(packed[11]), float(packed[12])),
            (0.0, 1.0),
            "no cone"
        );

        let mut bytes = Vec::new();
        Light::Directional(DirectionalLight {
            direction: Vec3::new(0.0, 0.0, -5.0),
            ..DirectionalLight::default()
        })
        .push_values(&mut bytes);
        let packed = words(&bytes);
        assert_eq!(u32::from_le_bytes(packed[3]), DIRECTIONAL);
        assert_eq!(
            [8, 9, 10].map(|index| float(packed[index])),
            [0.0, 0.0, -1.0]
        );
    }

    #[test]
    fn a_cone_is_full_at_its_inner_angle_and_empty_at_its_outer_one() {
        let (inner, outer) = (0.3_f32, 0.7_f32);
        let (scale, offset) = cone_scale_and_offset(inner, outer);
        assert!((inner.cos() * scale + offset - 1.0).abs() < 1e-5);
        assert!((outer.cos() * scale + offset).abs() < 1e-5);
        // Equal angles keep a finite, steep edge.
        let (scale, offset) = cone_scale_and_offset(0.5, 0.5);
        assert!(
            (scale - 1000.0).abs() < 0.01 && offset.is_finite(),
            "{scale} {offset}"
        );
    }
}
