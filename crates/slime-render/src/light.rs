//! Lights: what lights the surfaces of a scene, in the units of glTF's `KHR_lights_punctual`
//! (ADR-0037).

use std::f32::consts::FRAC_PI_4;

use glam::Vec3;
use slime_gfx::LinearColor;

use crate::bytes::{push_f32s, push_u32s};

/// The most lights [`Renderer::render`](crate::Renderer::render) draws: it draws the first ones of
/// a [`Scene`](crate::Scene) that has more and leaves the others out, logging a warning the first
/// time a renderer meets such a scene. Every fragment of the scene pass evaluates every light it
/// draws (ADR-0037).
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
/// metres, as in glTF. [`Renderer::render`](crate::Renderer::render) panics on a light with a value
/// outside the range its documentation gives.
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
    /// length does not matter, but it is finite and not zero.
    pub direction: Vec3,
    /// What `illuminance` is multiplied by in each of red, green and blue: finite and not negative.
    /// glTF keeps each within 0 and 1, so that `illuminance` alone sets the strength. Alpha is
    /// ignored.
    pub color: LinearColor,
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
            color: LinearColor::WHITE,
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
    /// Where the light is, in world space. Finite.
    pub position: Vec3,
    /// What `intensity` is multiplied by in each of red, green and blue: finite and not negative.
    /// glTF keeps each within 0 and 1, so that `intensity` alone sets the strength. Alpha is
    /// ignored.
    pub color: LinearColor,
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
            color: LinearColor::WHITE,
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
    /// Where the light is, in world space. Finite.
    pub position: Vec3,
    /// The direction the light travels in along the cone's axis, in world space. Its length does
    /// not matter, but it is finite and not zero, as for [`DirectionalLight::direction`].
    pub direction: Vec3,
    /// As [`PointLight::color`].
    pub color: LinearColor,
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
            color: LinearColor::WHITE,
            intensity: 1.0,
            range: None,
            inner_cone_angle: 0.0,
            outer_cone_angle: FRAC_PI_4,
        }
    }
}

impl Light {
    /// Which value of the light is outside the range its documentation gives, if one is: a color
    /// or a strength that is not finite or is negative, a position that is not finite, a
    /// direction that is zero or not finite, a range that is not positive and finite, or a spot's
    /// cones out of order or wider than π / 2.
    pub(crate) fn invalid(&self) -> Option<String> {
        let color = |color: LinearColor| {
            let color = rgb(color);
            (!color.is_finite() || color.min_element() < 0.0)
                .then(|| format!("its color {color} is not finite and not negative"))
        };
        let strength = |name: &str, value: f32| {
            (!value.is_finite() || value < 0.0)
                .then(|| format!("its {name} {value} is not finite and not negative"))
        };
        let position = |position: Vec3| {
            (!position.is_finite()).then(|| format!("its position {position} is not finite"))
        };
        let direction = |direction: Vec3| {
            (!direction.is_finite() || direction == Vec3::ZERO)
                .then(|| format!("its direction {direction} is zero or not finite"))
        };
        let range = |range: Option<f32>| {
            range
                .filter(|&range| !(range.is_finite() && range > 0.0))
                .map(|range| format!("its range {range} is not positive and finite"))
        };
        let cones = |inner: f32, outer: f32| {
            let ordered = 0.0 <= inner && inner < outer && outer <= std::f32::consts::FRAC_PI_2;
            (!ordered).then(|| {
                format!(
                    "its cones of {inner} and {outer} radians are not 0 ≤ inner < outer ≤ π / 2"
                )
            })
        };
        match *self {
            Self::Directional(light) => color(light.color)
                .or_else(|| strength("illuminance", light.illuminance))
                .or_else(|| direction(light.direction)),
            Self::Point(light) => color(light.color)
                .or_else(|| strength("intensity", light.intensity))
                .or_else(|| position(light.position))
                .or_else(|| range(light.range)),
            Self::Spot(light) => color(light.color)
                .or_else(|| strength("intensity", light.intensity))
                .or_else(|| position(light.position))
                .or_else(|| direction(light.direction))
                .or_else(|| range(light.range))
                .or_else(|| cones(light.inner_cone_angle, light.outer_cone_angle)),
        }
    }

    /// Appends the values the scene shader reads for this light, laid out as `Light` in
    /// `shader.wgsl`: [`LIGHT_VALUES_SIZE`] bytes. What a kind of light does not use is zero, but
    /// for the cone: a scale of 0 and an offset of 1 leave a point light's light whole. The position
    /// is relative to `origin`, the camera's eye (ADR-0048), computed in `f64`.
    pub(crate) fn push_values(&self, bytes: &mut Vec<u8>, origin: Vec3) {
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
        let strength = rgb(color) * intensity;
        push_f32s(bytes, &strength.to_array());
        push_u32s(bytes, &[kind]);
        // 1 / range², so that the shader's (d / range)⁴ is (d² / range²)², and 0 for no range.
        let inverse_range_squared = range.map_or(0.0, |range| 1.0 / (range * range));
        let position = (position.as_dvec3() - origin.as_dvec3()).as_vec3();
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

/// The red, green and blue of `color`, its alpha dropped: for the colors whose alpha is ignored,
/// the lights' and the sky's, in the vector math that light goes through.
pub(crate) fn rgb(color: LinearColor) -> Vec3 {
    Vec3::new(color.r, color.g, color.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_outside_their_documented_ranges_are_named() {
        let directional = DirectionalLight::default();
        let point = PointLight::default();
        let spot = SpotLight::default();
        for light in [
            Light::Directional(directional),
            Light::Point(point),
            Light::Spot(spot),
        ] {
            assert_eq!(light.invalid(), None, "{light:?}");
        }
        let cases = [
            (
                Light::Directional(DirectionalLight {
                    color: LinearColor::rgb(1.0, -0.1, 1.0),
                    ..directional
                }),
                "its color [1, -0.1, 1] is not finite and not negative",
            ),
            (
                Light::Directional(DirectionalLight {
                    illuminance: f32::NAN,
                    ..directional
                }),
                "its illuminance NaN is not finite and not negative",
            ),
            (
                Light::Directional(DirectionalLight {
                    direction: Vec3::ZERO,
                    ..directional
                }),
                "its direction [0, 0, 0] is zero or not finite",
            ),
            (
                Light::Point(PointLight {
                    intensity: -1.0,
                    ..point
                }),
                "its intensity -1 is not finite and not negative",
            ),
            (
                Light::Point(PointLight {
                    position: Vec3::new(f32::INFINITY, 0.0, 0.0),
                    ..point
                }),
                "its position [inf, 0, 0] is not finite",
            ),
            (
                Light::Point(PointLight {
                    range: Some(0.0),
                    ..point
                }),
                "its range 0 is not positive and finite",
            ),
            (
                Light::Spot(SpotLight {
                    direction: Vec3::new(0.0, f32::NAN, 0.0),
                    ..spot
                }),
                "its direction [0, NaN, 0] is zero or not finite",
            ),
            (
                Light::Spot(SpotLight {
                    inner_cone_angle: 0.5,
                    outer_cone_angle: 0.5,
                    ..spot
                }),
                "its cones of 0.5 and 0.5 radians are not 0 ≤ inner < outer ≤ π / 2",
            ),
            (
                Light::Spot(SpotLight {
                    outer_cone_angle: 2.0,
                    ..spot
                }),
                "its cones of 0 and 2 radians are not 0 ≤ inner < outer ≤ π / 2",
            ),
        ];
        for (light, reason) in cases {
            assert_eq!(light.invalid().as_deref(), Some(reason), "{light:?}");
        }
    }

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
        // A light 10 km from the world's origin, near the camera: packed relative to the camera.
        let mut bytes = Vec::new();
        Light::Spot(SpotLight {
            position: Vec3::new(10_001.0, 2.0, 3.0),
            direction: Vec3::new(0.0, -2.0, 0.0),
            color: LinearColor::rgb(1.0, 0.5, 0.25),
            intensity: 8.0,
            range: Some(4.0),
            inner_cone_angle: 0.0,
            outer_cone_angle: std::f32::consts::FRAC_PI_2,
        })
        .push_values(&mut bytes, Vec3::new(10_000.0, 0.0, 0.0));
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
        Light::Point(PointLight::default()).push_values(&mut bytes, Vec3::ZERO);
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
        .push_values(&mut bytes, Vec3::ZERO);
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
