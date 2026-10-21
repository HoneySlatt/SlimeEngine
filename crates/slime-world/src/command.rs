//! Commands: changes of the world asked for outside the phases of a frame that may make them, which
//! the host applies at the start of the next frame (`docs/adr/0107-the-execution-model.md`).

use slime_reflect::{Path, Value};

use crate::error::WorldError;
use crate::identity::PersistentId;
use crate::world::World;

/// A change of the world asked for outside the phases of a frame that may make it, by the dev
/// overlay, a tool or a game's presentation, which reads the world without writing it: the host
/// queues it and applies it at the start of the next frame ([`World::apply`],
/// `docs/adr/0107-the-execution-model.md`). It names its entity by its persistent identity, which
/// holds from one run to the next and after a load or a hot reload replaced the world, where a
/// handle would name another entity, or none (`docs/adr/0109-stable-references-and-undo.md`).
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Sets the part of a component that `path` leads to, as [`World::set_property`] does.
    SetProperty {
        /// The entity whose component changes, by its persistent identity.
        entity: PersistentId,
        /// The type of the component, by its stable name or an alias.
        component: String,
        /// The part of the component that changes; empty for the whole component.
        path: Path,
        /// The part's new value.
        value: Value,
    },
}

impl World {
    /// Applies `command`: what a host does at the start of each frame with the commands queued
    /// since the last one.
    ///
    /// ```
    /// use slime_color::LinearColor;
    /// use slime_reflect::Value;
    /// use slime_world::{Command, Light, World};
    ///
    /// let mut world = World::new();
    /// let lantern = world.spawn_keyed("lantern").unwrap();
    /// let light = Light::Point { color: LinearColor::WHITE, intensity: 30.0, range: None };
    /// world.insert(lantern, light);
    /// let dim = Command::SetProperty {
    ///     entity: "lantern".parse().unwrap(),
    ///     component: "Light".to_owned(),
    ///     path: "intensity".parse().unwrap(),
    ///     value: Value::F32(12.0),
    /// };
    /// world.apply(&dim).expect("the lantern is alive, and 12 is in range");
    /// assert!(matches!(world.get::<Light>(lantern), Some(Light::Point { intensity: 12.0, .. })));
    /// ```
    ///
    /// # Errors
    ///
    /// [`WorldError::NotFound`] when no entity alive has the command's persistent identity, and
    /// otherwise the errors of the method that it names. Nothing changes then.
    pub fn apply(&mut self, command: &Command) -> Result<(), WorldError> {
        match command {
            Command::SetProperty {
                entity,
                component,
                path,
                value,
            } => {
                let found = self
                    .find(entity)
                    .ok_or_else(|| WorldError::NotFound(entity.clone()))?;
                self.set_property(found, component, path, value.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use slime_color::LinearColor;

    use super::*;
    use crate::components::Light;

    fn dim(entity: &str, intensity: f32) -> Command {
        Command::SetProperty {
            entity: entity.parse().expect("an identity"),
            component: "Light".to_owned(),
            path: "intensity".parse().expect("a path"),
            value: Value::F32(intensity),
        }
    }

    #[test]
    fn a_command_names_its_entity_by_identity_and_is_refused_whole() {
        let mut world = World::new();
        let lantern = world.spawn();
        let point = Light::Point {
            color: LinearColor::WHITE,
            intensity: 30.0,
            range: None,
        };
        world.insert(lantern, point);
        let id = world.persistent_id(lantern).expect("created/0").to_string();
        world.apply(&dim(&id, 5.0)).expect("applied");
        assert!(matches!(
            world.get::<Light>(lantern),
            Some(Light::Point { intensity: 5.0, .. })
        ));
        assert!(matches!(
            world.apply(&dim("lantern", 5.0)),
            Err(WorldError::NotFound(id)) if id.to_string() == "lantern"
        ));
        assert!(matches!(
            world.apply(&dim(&id, -1.0)),
            Err(WorldError::Refused { .. })
        ));
        world.despawn(lantern);
        assert!(matches!(
            world.apply(&dim(&id, 5.0)),
            Err(WorldError::NotFound(_))
        ));
    }
}
