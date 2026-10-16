//! The engine's components for what a cooked scene's nodes hold.

use slime_assets::{AssetId, Extra, SceneLight};

use crate::Component;

/// An entity's name, as a scene file gives it: for people and logs. An entity's identity in files
/// and saves is its [`PersistentId`](crate::PersistentId), not its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name(pub String);

impl Component for Name {}

/// The cooked meshes an entity places, each with its material, by their asset IDs: one per
/// primitive of a glTF node's mesh.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meshes(pub Vec<AssetId>);

impl Component for Meshes {}

/// The light an entity places, in its own space: a cooked scene's node light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light(pub SceneLight);

impl Component for Light {}

/// The custom values of an entity, by name, as a cooked scene's node carried them: what a game
/// reads its own data from, such as colliders and markers.
#[derive(Clone, Debug, PartialEq)]
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
