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

/// An entity that holds it is left out of the world's saves, and so are its descendants: what a
/// game creates for a moment, such as an effect or a projectile, which a run resumed from a save
/// does without (`docs/adr/0087-the-worlds-documents.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[reflect(name = "Unsaved")]
pub struct Unsaved;

impl Component for Unsaved {}

/// What becomes of what a run changed of a scene's entity when the scene is unloaded
/// ([`World::unload_scene`](crate::World::unload_scene)): held by a scene's root, for its scene's
/// entities; held by an entity, for itself, whatever its scene's. The entities of a scene whose
/// root holds none are persistent (`docs/adr/0125-saves-per-scene.md`). It is read from the entity
/// as it holds it when the scene is unloaded, or held it when the run destroyed it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(name = "Persistence")]
pub enum Persistence {
    /// Kept: the world keeps what the run changed of the entity, and applies it again when the
    /// scene is spawned again, as a save does, a door broken open, a chest emptied.
    #[default]
    Persistent,
    /// Forgotten: spawned again, the scene gives the entity as its file does, an enemy that comes
    /// back. What the run changed of it is saved while its scene is loaded.
    Reset,
}

impl Component for Persistence {}

/// The components of an entity that a system computes again, the game's rules, an animation or
/// physics, by their types' names, `Transform`: their value at a given moment does not matter, so
/// that a save, and a scene file written back, leave each as its author wrote it, never its value of
/// the moment, which the system computes again after a load
/// (`docs/adr/0108-what-the-world-knows-of-its-changes.md`,
/// `docs/adr/0124-driven-components-keys-and-created-entities-in-levels.md`). A driven component
/// that no author wrote, on an entity that a run created or added to one, is saved as it is:
/// nothing else gives it.
///
/// The mark is authored, as a scene file holds it: a save leaves out the changes of a component
/// that both its entity's base and the run mark, so that a mark that the run adds, or takes off,
/// hides nothing. It is not inherited: it marks its own entity's components only. A component is
/// driven whole: a game keeps what a player changes, a lamp switched off, in another component than
/// the one it drives, the lamp's flicker. It names the types as the world's registry names them,
/// `Transform`, `hunnawood::Look`, not by their aliases; a name of no component that the entity
/// holds marks nothing.
#[derive(Clone, Debug, PartialEq, Eq, Reflect)]
#[reflect(name = "Driven")]
pub struct Driven(pub Vec<String>);

impl Driven {
    /// Whether it marks the component of the type named `name` driven.
    pub fn holds(&self, name: &str) -> bool {
        self.0.iter().any(|driven| driven == name)
    }
}

impl Component for Driven {}

/// The light an entity places: at its origin, along its −Z axis, whatever its scale; in the
/// physical units of glTF's `KHR_lights_punctual`, as the renderer's lights are, by the same rule,
/// [`SceneLight::check`], and with a shadow distance positive and finite.
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
    /// Accepts the light if the engine can draw it, by the rule of a cooked scene's lights
    /// ([`SceneLight::check`]), its shadow distance positive and finite; refuses it with the reason
    /// otherwise: what reflection checks when it reads a light (`#[reflect(check)]`), the rules
    /// between fields and on the color included, which ranges cannot say.
    fn check(&self) -> Result<(), String> {
        let (color, intensity, range, kind) = match *self {
            Self::Directional {
                color,
                illuminance,
                shadow_distance,
                ..
            } => {
                if let Some(distance) = shadow_distance
                    && !(distance.is_finite() && distance > 0.0)
                {
                    return Err(format!(
                        "its shadow distance {distance} is not positive and finite"
                    ));
                }
                (color, illuminance, None, LightKind::Directional)
            }
            Self::Point {
                color,
                intensity,
                range,
            } => (color, intensity, range, LightKind::Point),
            Self::Spot {
                color,
                intensity,
                range,
                inner_cone_angle,
                outer_cone_angle,
                ..
            } => (
                color,
                intensity,
                range,
                LightKind::Spot {
                    inner_cone_angle,
                    outer_cone_angle,
                },
            ),
        };
        SceneLight {
            kind,
            color: [color.r, color.g, color.b],
            intensity,
            range,
        }
        .check()
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
