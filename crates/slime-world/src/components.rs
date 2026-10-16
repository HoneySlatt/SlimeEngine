//! The engine's components: what a cooked scene's nodes hold, and what the world draws
//! (`docs/adr/0084-what-the-world-draws.md`).

use slime_assets::{AssetId, Extra, LightKind, SceneLight};
use slime_color::LinearColor;

use slime_reflect::Reflect;

use crate::Component;

/// An entity's name, as a scene file gives it: for people and logs. An entity's identity in files
/// and saves is its [`PersistentId`](crate::PersistentId), not its name.
#[derive(Clone, Debug, PartialEq, Eq, Reflect)]
#[reflect(name = "Name")]
pub struct Name(pub String);

impl Component for Name {}

/// The cooked meshes an entity places, by their asset IDs, one per primitive of a glTF node's mesh:
/// each is drawn with the material that its cooked mesh names, where the entity's [`Transform`]
/// places it in the world, its stretch included.
///
/// [`Transform`]: crate::Transform
#[derive(Clone, Debug, PartialEq, Eq, Reflect)]
#[reflect(name = "Meshes")]
pub struct Meshes {
    /// The meshes, by their asset IDs.
    pub ids: Vec<AssetId>,
    /// Whether they cast shadows; they receive them either way.
    pub casts_shadows: bool,
}

impl Component for Meshes {}

/// An entity that holds it is not drawn, nor are its descendants, nor the lights they place:
/// a door that opened, a key taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[reflect(name = "Hidden")]
pub struct Hidden;

impl Component for Hidden {}

/// The light an entity places: at its origin, along its −Z axis, whatever its scale; in the
/// physical units of glTF's `KHR_lights_punctual`, as the renderer's lights are, with the same
/// ranges: a color and a strength finite and not negative, a range positive, a spot's cones
/// within 0 ≤ inner < outer ≤ π / 2.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "Light", check = Light::check)]
pub enum Light {
    /// A light from one direction everywhere, like the sun.
    Directional {
        /// What `illuminance` is multiplied by in each of red, green and blue; alpha is ignored.
        color: LinearColor,
        /// The illuminance it gives a surface facing it, in lux.
        #[reflect(range(min = 0.0), unit = "lx")]
        illuminance: f32,
        /// Whether it casts shadows; the renderer draws those of one directional light.
        casts_shadows: bool,
        /// How far from the camera it casts shadows, in metres; `None` for the renderer's default.
        #[reflect(range(above = 0.0), unit = "m")]
        shadow_distance: Option<f32>,
        /// Whether it shows in the sky, with its disc and its glow, as the sky's sun: the first
        /// directional light drawn that does is the sun (`docs/adr/0084-what-the-world-draws.md`).
        /// A sun hidden by clouds lights the scene without showing in the sky.
        in_sky: bool,
    },
    /// A light from a point, in every direction.
    Point {
        /// What `intensity` is multiplied by in each of red, green and blue; alpha is ignored.
        color: LinearColor,
        /// Its luminous intensity, in candelas.
        #[reflect(range(min = 0.0), unit = "cd")]
        intensity: f32,
        /// The distance at and beyond which it reaches nothing, in metres; `None` for none.
        #[reflect(range(above = 0.0), unit = "m")]
        range: Option<f32>,
    },
    /// A light from a point, in a cone along the entity's −Z axis.
    Spot {
        /// What `intensity` is multiplied by in each of red, green and blue; alpha is ignored.
        color: LinearColor,
        /// Its luminous intensity inside the inner cone, in candelas.
        #[reflect(range(min = 0.0), unit = "cd")]
        intensity: f32,
        /// The distance at and beyond which it reaches nothing, in metres; `None` for none.
        #[reflect(range(above = 0.0), unit = "m")]
        range: Option<f32>,
        /// The angle from the axis within which the light is full, in radians.
        #[reflect(range(min = 0.0, max = std::f32::consts::FRAC_PI_2), unit = "rad")]
        inner_cone_angle: f32,
        /// The angle from the axis beyond which there is no light, in radians.
        #[reflect(range(min = 0.0, max = std::f32::consts::FRAC_PI_2), unit = "rad")]
        outer_cone_angle: f32,
        /// Whether it casts shadows.
        casts_shadows: bool,
    },
}

impl Light {
    /// Accepts the light if its values lie within the ranges its documentation gives, which the
    /// renderer refuses a light outside of; refuses it with the reason otherwise: what reflection
    /// checks when it reads a light (`#[reflect(check)]`), the rules between fields and on the
    /// color included, which ranges cannot say.
    fn check(&self) -> Result<(), String> {
        let color = |color: &LinearColor| {
            let ok = [color.r, color.g, color.b]
                .into_iter()
                .all(|channel| channel.is_finite() && channel >= 0.0);
            ok.then_some(())
                .ok_or_else(|| format!("its color {color:?} is negative or not finite"))
        };
        let strength = |name: &str, value: f32| {
            (value.is_finite() && value >= 0.0)
                .then_some(())
                .ok_or_else(|| format!("its {name} {value} is negative or not finite"))
        };
        let distance = |name: &str, value: Option<f32>| match value {
            Some(value) if !(value.is_finite() && value > 0.0) => {
                Err(format!("its {name} {value} is not positive and finite"))
            }
            _ => Ok(()),
        };
        match self {
            Self::Directional {
                color: tint,
                illuminance,
                shadow_distance,
                ..
            } => {
                color(tint)?;
                strength("illuminance", *illuminance)?;
                distance("shadow distance", *shadow_distance)
            }
            Self::Point {
                color: tint,
                intensity,
                range,
            } => {
                color(tint)?;
                strength("intensity", *intensity)?;
                distance("range", *range)
            }
            Self::Spot {
                color: tint,
                intensity,
                range,
                inner_cone_angle: inner,
                outer_cone_angle: outer,
                ..
            } => {
                color(tint)?;
                strength("intensity", *intensity)?;
                distance("range", *range)?;
                (0.0 <= *inner && inner < outer && *outer <= std::f32::consts::FRAC_PI_2)
                    .then_some(())
                    .ok_or_else(|| {
                        format!(
                            "its cones of {inner} and {outer} radians are not \
                             0 ≤ inner < outer ≤ π / 2"
                        )
                    })
            }
        }
    }

    /// The light of a cooked scene's node: casting shadows if it can, a directional one not in the
    /// sky.
    pub fn from_cooked(light: &SceneLight) -> Self {
        let [r, g, b] = light.color;
        let color = LinearColor::rgb(r, g, b);
        match light.kind {
            LightKind::Directional => Self::Directional {
                color,
                illuminance: light.intensity,
                casts_shadows: true,
                shadow_distance: None,
                in_sky: false,
            },
            LightKind::Point => Self::Point {
                color,
                intensity: light.intensity,
                range: light.range,
            },
            LightKind::Spot {
                inner_cone_angle,
                outer_cone_angle,
            } => Self::Spot {
                color,
                intensity: light.intensity,
                range: light.range,
                inner_cone_angle,
                outer_cone_angle,
                casts_shadows: true,
            },
        }
    }
}

impl Component for Light {}

/// The custom values of an entity, by name, as a cooked scene's node carried them: what a game
/// reads its own data from, such as colliders and markers.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "Extras")]
pub struct Extras(pub Vec<(String, Extra)>);

impl Extras {
    /// The value of the custom value named `name`, if there is one.
    pub fn get(&self, name: &str) -> Option<&Extra> {
        self.0
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }
}

impl Component for Extras {}
