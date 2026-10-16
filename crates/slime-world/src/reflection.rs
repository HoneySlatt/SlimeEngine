//! The components of a world reached by name, through reflection, without their types
//! (`docs/adr/0085-reflection.md`).

use std::any::TypeId;

use slime_reflect::{Path, Reflect, Registry, Value};

use crate::entity::Entity;
use crate::error::WorldError;
use crate::transform::Transform;
use crate::world::{World, mark_stale};

impl World {
    /// The types of the world's components, and the types their values hold: the engine's, every
    /// type of which the world has stored a component, and those registered ahead
    /// ([`World::register`]).
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Registers `T` and the types its values hold, ahead of any component of `T` that the world
    /// stores ([`World::insert`] registers its type then): what a game does for the types it must
    /// find by name before an entity holds one, such as the components of a save it reads.
    ///
    /// # Panics
    ///
    /// As [`Registry::register`]: if another type has `T`'s name.
    pub fn register<T: Reflect>(&mut self) {
        self.registry.register::<T>();
    }

    /// Every entity alive, in the order of their slots.
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entities.iter()
    }

    /// The names of the types of the components that `entity` holds, in the order the world first
    /// stored a component of each; none if it is not alive.
    pub fn component_names(&self, entity: Entity) -> Vec<&str> {
        if !self.contains(entity) {
            return Vec::new();
        }
        self.storages
            .iter()
            .zip(&self.component_types)
            .filter(|(storage, _)| storage.holds(entity.index()))
            .filter_map(|(_, &type_id)| self.registry.of_type(type_id))
            .map(|registration| registration.info().name.as_str())
            .collect()
    }

    /// The value of `entity`'s component whose type is named `component`, by its name or an alias,
    /// if the entity is alive and holds one.
    ///
    /// ```
    /// use slime_reflect::Value;
    /// use slime_world::{Name, World};
    ///
    /// let mut world = World::new();
    /// let door = world.spawn();
    /// world.insert(door, Name("door".to_owned()));
    /// let name = world.component_value(door, "Name").expect("held");
    /// assert_eq!(name.get(&"0".parse().unwrap()), Some(&Value::Text("door".to_owned())));
    /// assert_eq!(world.component_names(door), ["Name"]);
    /// ```
    pub fn component_value(&self, entity: Entity, component: &str) -> Option<Value> {
        let (id, _) = self.held(entity, component).ok()?;
        self.storages[id].value(entity.index())
    }

    /// Replaces `entity`'s component whose type is named `component` with the one that `value`
    /// describes, once reflection has checked it, its numbers within its fields' ranges
    /// ([`Registry::set`]). A [`Transform`] set this way counts as changed.
    ///
    /// # Errors
    ///
    /// [`WorldError::NoComponent`] if the entity is not alive or holds no such component;
    /// [`WorldError::Refused`] if `value` is not a value of the component's type, or holds a
    /// number out of its field's range, with the path of the part at fault. The component stays as
    /// it was then.
    pub fn set_component_value(
        &mut self,
        entity: Entity,
        component: &str,
        value: Value,
    ) -> Result<(), WorldError> {
        self.set_property(entity, component, &Path::default(), value)
    }

    /// Sets the part of `entity`'s component whose type is named `component` that `path` leads to,
    /// to `value`, once reflection has checked it, as [`Registry::set`] does: a field of the
    /// component, an element of a list, a field of the variant an enum holds. A [`Transform`]
    /// changed this way counts as changed.
    ///
    /// ```
    /// use slime_color::LinearColor;
    /// use slime_reflect::Value;
    /// use slime_world::{Light, World};
    ///
    /// let mut world = World::new();
    /// let lantern = world.spawn();
    /// world.insert(lantern, Light::Point { color: LinearColor::WHITE, intensity: 30.0, range: None });
    /// let intensity = "intensity".parse().unwrap();
    /// world.set_property(lantern, "Light", &intensity, Value::F32(12.0)).expect("in range");
    /// assert!(matches!(world.get::<Light>(lantern), Some(Light::Point { intensity: 12.0, .. })));
    /// let refused = world.set_property(lantern, "Light", &intensity, Value::F32(-1.0));
    /// assert_eq!(
    ///     refused.unwrap_err().to_string(),
    ///     "component `Light`: `intensity`: -1 is not ≥ 0"
    /// );
    /// ```
    ///
    /// # Errors
    ///
    /// As [`World::set_component_value`], and [`WorldError::Refused`] when `path` names no part of
    /// the component.
    pub fn set_property(
        &mut self,
        entity: Entity,
        component: &str,
        path: &Path,
        value: Value,
    ) -> Result<(), WorldError> {
        let (id, name) = self.held(entity, component)?;
        let refused = |error| WorldError::Refused {
            component: name.clone(),
            error,
        };
        let slot = entity.index();
        let mut current = self.storages[id]
            .value(slot)
            .ok_or_else(|| WorldError::NoComponent {
                entity,
                component: component.to_owned(),
            })?;
        self.registry
            .set(&name, &mut current, path, value)
            .map_err(refused)?;
        self.storages[id]
            .set_value(slot, &current)
            .map_err(refused)?;
        if self.component_types[id] == TypeId::of::<Transform>() {
            mark_stale(&mut self.nodes, &mut self.changes, slot);
        }
        Ok(())
    }

    /// The component ID of `entity`'s component whose type is named `component`, and the type's
    /// name.
    fn held(&self, entity: Entity, component: &str) -> Result<(usize, String), WorldError> {
        let none = || WorldError::NoComponent {
            entity,
            component: component.to_owned(),
        };
        if !self.contains(entity) {
            return Err(none());
        }
        let registration = self.registry.get(component).ok_or_else(none)?;
        let type_id = registration.type_id().ok_or_else(none)?;
        let &id = self.component_ids.get(&type_id).ok_or_else(none)?;
        if self.storages[id].holds(entity.index()) {
            Ok((id, registration.info().name.clone()))
        } else {
            Err(none())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use glam::{DVec3, Quat, Vec3};
    use slime_assets::{AssetId, Extra};
    use slime_color::LinearColor;
    use slime_reflect::{Kind, Leaf, Problem, Range};

    use super::*;
    use crate::{Component, Extras, Hidden, Light, Meshes, Name};

    /// A component that only derives, to show that the derive is all a component needs.
    #[derive(Clone, Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Lantern")]
    struct Lantern {
        /// How strong its light is.
        #[reflect(range(min = 0.0), unit = "cd")]
        intensity: f32,
        lit: bool,
        label: String,
        wicks: Vec<u8>,
        color: Option<LinearColor>,
    }
    impl Component for Lantern {}

    /// A number of `range`, other than `not`.
    fn pick(range: Range, not: f64) -> f64 {
        [1.0, 0.5, 2.0, 0.25, range.min + 1.0, range.max - 0.125]
            .into_iter()
            .find(|&candidate| candidate != not && range.contains(candidate))
            .expect("the range holds another number")
    }

    /// A value of the type named `ty`, within `range`, for an option that holds none.
    fn sample(registry: &Registry, ty: &str, range: Range) -> Option<Value> {
        match &registry.get(ty)?.info().kind {
            Kind::F32 => Some(Value::F32(pick(range, f64::NAN) as f32)),
            Kind::F64 => Some(Value::F64(pick(range, f64::NAN))),
            Kind::Struct(fields) => fields
                .iter()
                .map(|field| Some((field.name.clone(), sample(registry, &field.ty, Range::ALL)?)))
                .collect::<Option<Vec<_>>>()
                .map(Value::Struct),
            _ => None,
        }
    }

    /// Another value for `leaf`, which holds `value`, of its type and within its field's range:
    /// what an edit of it sets. `None` for a leaf that holds an entity, or an empty list.
    fn another(registry: &Registry, leaf: &Leaf, value: &Value) -> Option<Value> {
        let range = leaf
            .field
            .as_ref()
            .and_then(|f| f.range)
            .unwrap_or(Range::ALL);
        Some(match value {
            Value::Bool(value) => Value::Bool(!value),
            Value::Int(value) => Value::Int(value + 1),
            Value::UInt(value) => Value::UInt(value + 1),
            Value::F32(value) => Value::F32(pick(range, f64::from(*value)) as f32),
            Value::F64(value) => Value::F64(pick(range, *value)),
            Value::Text(text) => Value::Text(format!("{text}!")),
            Value::Asset(id) => Value::Asset(id + 1),
            Value::Option(Some(_)) => Value::Option(None),
            Value::Option(None) => {
                let Kind::Option(inner) = &registry.get(&leaf.ty)?.info().kind else {
                    return None;
                };
                Value::Option(Some(Box::new(sample(registry, inner, range)?)))
            }
            Value::Enum(variant, _) => {
                let Kind::Enum(variants) = &registry.get(&leaf.ty)?.info().kind else {
                    return None;
                };
                let next = variants
                    .iter()
                    .find(|other| &other.name != variant && other.fields.is_empty())?;
                Value::Enum(next.name.clone(), Vec::new())
            }
            _ => return None,
        })
    }

    /// Sets each leaf of `entity`'s component of `T` to another value, through its path, and
    /// checks that it reads back, through reflection and typed.
    fn every_leaf_is_set_and_read_back<T: Component + Debug>(world: &mut World, entity: Entity) {
        let name = T::type_name();
        let value = world.component_value(entity, &name).expect("held");
        let leaves = world
            .registry()
            .leaves(&name, &value)
            .expect("a value of its type");
        for leaf in leaves {
            let before = world.component_value(entity, &name).expect("held");
            let at = before.get(&leaf.path).expect("a leaf of the value").clone();
            let Some(new) = another(world.registry(), &leaf, &at) else {
                continue;
            };
            world
                .set_property(entity, &name, &leaf.path, new.clone())
                .unwrap_or_else(|error| panic!("{name}.{}: {error}", leaf.path));
            let after = world.component_value(entity, &name).expect("held");
            assert_eq!(after.get(&leaf.path), Some(&new), "{name}.{}", leaf.path);
            let typed = world.get::<T>(entity).expect("held").to_value();
            assert_eq!(typed, after, "{name}.{}: the typed component", leaf.path);
        }
    }

    /// An entity that holds one of each of the engine's components, and a lantern.
    fn furnished(world: &mut World) -> Entity {
        let entity = world.spawn();
        world.insert(
            entity,
            Transform {
                translation: DVec3::new(1.0, 2.0, 3.0),
                rotation: Quat::from_rotation_y(0.5),
                scale: 2.0,
                stretch: Vec3::new(1.0, 0.5, 1.0),
            },
        );
        world.insert(entity, Name("lamp post".to_owned()));
        world.insert(
            entity,
            Meshes {
                ids: vec![AssetId::from_u128(7), AssetId::from_u128(8)],
                casts_shadows: true,
            },
        );
        world.insert(entity, Hidden);
        world.insert(
            entity,
            Light::Spot {
                color: LinearColor::rgb(1.0, 0.8, 0.6),
                intensity: 30.0,
                range: Some(8.0),
                inner_cone_angle: 0.3,
                outer_cone_angle: 0.6,
                casts_shadows: true,
            },
        );
        world.insert(
            entity,
            Extras(vec![
                ("marker".to_owned(), Extra::Text("door".to_owned())),
                ("height".to_owned(), Extra::Number(2.5)),
            ]),
        );
        world.insert(
            entity,
            Lantern {
                intensity: 12.0,
                lit: true,
                label: "north".to_owned(),
                wicks: vec![3],
                color: None,
            },
        );
        entity
    }

    #[test]
    fn every_leaf_of_every_component_is_set_through_its_path_and_read_back() {
        let mut world = World::new();
        let entity = furnished(&mut world);
        every_leaf_is_set_and_read_back::<Transform>(&mut world, entity);
        every_leaf_is_set_and_read_back::<Name>(&mut world, entity);
        every_leaf_is_set_and_read_back::<Meshes>(&mut world, entity);
        every_leaf_is_set_and_read_back::<Hidden>(&mut world, entity);
        every_leaf_is_set_and_read_back::<Light>(&mut world, entity);
        every_leaf_is_set_and_read_back::<Extras>(&mut world, entity);
        every_leaf_is_set_and_read_back::<Lantern>(&mut world, entity);
        // The point and directional lights' fields too.
        for light in [
            Light::Point {
                color: LinearColor::WHITE,
                intensity: 2.0,
                range: None,
            },
            Light::Directional {
                color: LinearColor::WHITE,
                illuminance: 3.0,
                casts_shadows: true,
                shadow_distance: None,
                in_sky: false,
            },
        ] {
            world.insert(entity, light);
            every_leaf_is_set_and_read_back::<Light>(&mut world, entity);
        }
    }

    #[test]
    fn the_engine_s_components_are_registered_and_listed_by_name() {
        let mut world = World::new();
        for name in ["Transform", "Name", "Meshes", "Hidden", "Light", "Extras"] {
            assert!(world.registry().get(name).is_some(), "{name}");
        }
        assert!(world.registry().get("test::Lantern").is_none());
        let entity = furnished(&mut world);
        assert!(
            world.registry().get("test::Lantern").is_some(),
            "stored, so registered"
        );
        // Every type lists its properties, each with a name and a registered type.
        for registration in world.registry().iter() {
            let fields: Vec<_> = match &registration.info().kind {
                Kind::Struct(fields) => fields.iter().collect(),
                Kind::Enum(variants) => variants.iter().flat_map(|v| &v.fields).collect(),
                _ => Vec::new(),
            };
            for field in fields {
                assert!(!field.name.is_empty(), "{}", registration.info().name);
                assert!(world.registry().get(&field.ty).is_some(), "{}", field.ty);
            }
        }
        let Kind::Enum(variants) = &world
            .registry()
            .get("Light")
            .expect("registered")
            .info()
            .kind
        else {
            panic!("an enum");
        };
        let intensity = &variants[1].fields[1];
        assert_eq!(
            (intensity.name.as_str(), intensity.unit.as_deref()),
            ("intensity", Some("cd"))
        );
        assert_eq!(intensity.doc, "Its luminous intensity, in candelas.");
        assert_eq!(
            world.component_names(entity),
            [
                "Transform",
                "Name",
                "Meshes",
                "Hidden",
                "Light",
                "Extras",
                "test::Lantern"
            ]
        );
        let second = world.spawn();
        world.despawn(entity);
        assert_eq!(world.entities().collect::<Vec<_>>(), [second]);
        assert!(world.component_names(entity).is_empty());
        // Registered ahead, before any entity holds one.
        #[derive(Reflect)]
        #[reflect(name = "test::Saved")]
        struct Saved(u32);
        world.register::<Saved>();
        assert!(world.registry().get("test::Saved").is_some());
    }

    #[test]
    fn components_round_trip_through_their_values() {
        let mut world = World::new();
        let entity = furnished(&mut world);
        fn round_trip<T: Component + PartialEq + Debug>(world: &World, entity: Entity) {
            let held = world.get::<T>(entity).expect("held");
            assert_eq!(T::from_value(&held.to_value()).as_ref(), Ok(held));
        }
        round_trip::<Transform>(&world, entity);
        round_trip::<Name>(&world, entity);
        round_trip::<Meshes>(&world, entity);
        round_trip::<Hidden>(&world, entity);
        round_trip::<Light>(&world, entity);
        round_trip::<Extras>(&world, entity);
        round_trip::<Lantern>(&world, entity);
        assert_eq!(Entity::from_value(&entity.to_value()), Ok(entity));
        assert!(
            Entity::from_value(&Value::Entity(5)).is_err(),
            "no generation"
        );
    }

    #[test]
    fn a_value_that_a_component_refuses_names_the_component_and_the_path() {
        let mut world = World::new();
        let entity = furnished(&mut world);
        let path = |text: &str| text.parse::<Path>().expect("a path");
        let refusals = [
            (
                "Light",
                "intensity",
                Value::F32(-1.0),
                "`intensity`: -1 is not ≥ 0",
            ),
            (
                "Light",
                "inner_cone_angle",
                Value::F32(2.0),
                "`inner_cone_angle`: 2 is not from 0",
            ),
            (
                "Light",
                "illuminance",
                Value::F32(1.0),
                "`illuminance`: names nothing",
            ),
            (
                "Transform",
                "translation.x",
                Value::Text("east".to_owned()),
                "a value of `f64`",
            ),
            (
                "Meshes",
                "ids[5]",
                Value::Asset(1),
                "`ids[5]`: names nothing",
            ),
        ];
        for (component, at, value, message) in refusals {
            let before = world.component_value(entity, component);
            let error = world
                .set_property(entity, component, &path(at), value)
                .expect_err(message);
            assert!(matches!(error, WorldError::Refused { .. }), "{error}");
            assert!(error.to_string().contains(message), "{error}");
            assert_eq!(world.component_value(entity, component), before);
        }
        let error = world
            .set_property(entity, "Wind", &path("speed"), Value::F32(1.0))
            .expect_err("no such component");
        assert!(matches!(error, WorldError::NoComponent { .. }));
        let other = world.spawn();
        assert!(
            world
                .set_component_value(other, "Name", Name("x".to_owned()).to_value())
                .is_err()
        );
        let error = world
            .set_component_value(entity, "Name", Value::Text("bare".to_owned()))
            .expect_err("a name is a struct of its text");
        let WorldError::Refused { error, .. } = error else {
            panic!("refused");
        };
        assert!(matches!(error.problem(), Problem::WrongKind { .. }));
    }

    #[test]
    fn a_transform_set_through_reflection_moves_its_entity_and_its_children() {
        let mut world = World::new();
        let parent = world.spawn();
        world.insert(parent, Transform::IDENTITY);
        let child = world.spawn();
        world.insert(child, Transform::from_translation(DVec3::X));
        world.set_parent(child, Some(parent)).expect("alive");
        world.update_transforms();
        world
            .set_property(
                parent,
                "Transform",
                &"translation.z".parse().unwrap(),
                Value::F64(5.0),
            )
            .expect("set");
        let at = world.global_transform(child).expect("placed").translation;
        assert_eq!(at, DVec3::new(1.0, 0.0, 5.0));
    }

    #[test]
    fn differences_between_two_components_give_one_back_from_the_other() {
        let mut world = World::new();
        let entity = furnished(&mut world);
        let before = world.component_value(entity, "Light").expect("held");
        world
            .set_property(
                entity,
                "Light",
                &"range".parse().unwrap(),
                Value::Option(None),
            )
            .expect("set");
        let after = world.component_value(entity, "Light").expect("held");
        let changes = slime_reflect::diff(&before, &after);
        assert_eq!(changes.len(), 1);
        let mut applied = before.clone();
        slime_reflect::apply(&mut applied, &changes).expect("applied");
        assert_eq!(applied, after);
    }
}
