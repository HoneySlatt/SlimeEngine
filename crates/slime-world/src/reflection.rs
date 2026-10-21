//! The components of a world reached by name, through reflection, without their types
//! (`docs/adr/0085-reflection.md`).

use std::any::TypeId;

use slime_reflect::{Path, Registry, Value};

use crate::entity::Entity;
use crate::error::WorldError;
use crate::transform::Transform;
use crate::world::{Component, ComponentIds, World};

impl World {
    /// The types of the world's components, and the types their values hold: the engine's, every
    /// type of which the world has stored a component, and those registered ahead
    /// ([`World::register`]).
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Registers the component type `T` and the types its values hold, and makes its storage, ahead
    /// of any component of `T` that the world stores ([`World::insert`] does so then): what a game
    /// does for the types it must find by name before an entity holds one, such as the components
    /// that a save adds ([`World::insert_component_value`]).
    ///
    /// # Panics
    ///
    /// As [`Registry::register`]: if another type has `T`'s name.
    pub fn register<T: Component>(&mut self) {
        self.storage_id::<T>();
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
    /// ([`Registry::set`]), and the component's type has accepted it, as it accepts every value it
    /// reads (`#[reflect(check)]`). The component is stamped as changed ([`World::changed`]), and a
    /// [`Transform`] set this way marks its subtree to be computed again.
    ///
    /// # Errors
    ///
    /// [`WorldError::NoComponent`] if the entity is not alive or holds no such component;
    /// [`WorldError::Refused`] if `value` is not a value of the component's type, holds a number out
    /// of its field's range, or breaks a rule of the type's check, with the path of the part at
    /// fault. The component stays as it was then.
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
    /// component, an element of a list, a field of the variant an enum holds, through an option
    /// that holds a value. The component is read again whole from its value, which its type's
    /// check accepts or refuses: a field that the type skips takes its `Default` value. The
    /// component is stamped as changed, as [`World::set_component_value`] says.
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
        let tick = self.next_tick();
        let base = self.nodes[slot as usize].base;
        self.storages[id]
            .set_value(slot, &current, tick, base)
            .map_err(refused)?;
        self.held_changed(self.component_types[id], slot);
        Ok(())
    }

    /// Attaches to `entity` the component whose type is named `component`, by its name or an
    /// alias, that `value` describes, in place of the one of that type it holds, if it holds one:
    /// what a save that adds a component does. The type must have a storage in the world: one that
    /// it has stored, or registered ([`World::register`]). The value is checked as
    /// [`World::set_component_value`] checks it, and stamped as [`World::insert`] stamps it. A
    /// [`Transform`] attached so marks its subtree to be computed again, and one that the entity did
    /// not hold is drawn at once where it is, as [`World::insert`] does.
    ///
    /// # Errors
    ///
    /// [`WorldError::NotAlive`] if the entity is not alive; [`WorldError::UnknownComponent`] if no
    /// component type of that name has a storage in the world; [`WorldError::Refused`] if `value` is
    /// not a value of the type, holds a number out of its field's range, or breaks a rule of the
    /// type's check. Nothing changes then.
    pub fn insert_component_value(
        &mut self,
        entity: Entity,
        component: &str,
        value: Value,
    ) -> Result<(), WorldError> {
        if !self.contains(entity) {
            return Err(WorldError::NotAlive(entity));
        }
        let (id, name) = self.storage_named(component)?;
        let type_id = self.component_types[id];
        let refused = |error| WorldError::Refused {
            component: name.clone(),
            error,
        };
        // Checked as an edit of the whole component is, its numbers within their fields' ranges.
        let mut checked = Value::Bool(false);
        self.registry
            .set(&name, &mut checked, &Path::default(), value)
            .map_err(refused)?;
        let slot = entity.index();
        let held = self.storages[id].holds(slot);
        let tick = self.next_tick();
        let base = self.nodes[slot as usize].base;
        self.storages[id]
            .insert_value(slot, &checked, tick, base)
            .map_err(refused)?;
        self.held_changed(type_id, slot);
        if type_id == TypeId::of::<Transform>() && !held {
            self.teleport(entity);
        }
        Ok(())
    }

    /// Detaches from `entity` its component whose type is named `component`, by its name or an
    /// alias, and returns its value, if the entity is alive and held one: what a save that removes a
    /// component does. The world's log of removals tells it, as [`World::remove`] does.
    pub fn remove_component_value(&mut self, entity: Entity, component: &str) -> Option<Value> {
        let (id, _) = self.held(entity, component).ok()?;
        let slot = entity.index();
        let tick = self.next_tick();
        let base = self.nodes[slot as usize].base;
        let value = self.storages[id].remove_value(slot, entity, tick, base);
        self.held_changed(self.component_types[id], slot);
        value
    }

    /// The component ID, the index of its storage, of the type named `component`, by its name or an
    /// alias, and the type's name.
    ///
    /// # Errors
    ///
    /// [`WorldError::UnknownComponent`] if no component type of that name has a storage in the world:
    /// one that it has stored, or registered ([`World::register`]).
    pub(crate) fn storage_named(&self, component: &str) -> Result<(usize, String), WorldError> {
        storage_in_registry(&self.registry, &self.component_ids, component)
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

/// The component ID, the index of its storage, of the type named `component` in `registry`, by its
/// name or an alias, among a world's `component_ids`, and the type's name: what
/// [`World::storage_named`] gives, for a world whose registry is set apart while a save is read.
///
/// # Errors
///
/// As [`World::storage_named`].
pub(crate) fn storage_in_registry(
    registry: &Registry,
    component_ids: &ComponentIds,
    component: &str,
) -> Result<(usize, String), WorldError> {
    let unknown = || WorldError::UnknownComponent(component.to_owned());
    let registration = registry.get(component).ok_or_else(unknown)?;
    let type_id = registration.type_id().ok_or_else(unknown)?;
    let id = component_ids.get(&type_id).copied().ok_or_else(unknown)?;
    Ok((id, registration.info().name.clone()))
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use glam::{DVec3, Quat, Vec3};
    use slime_assets::{AssetId, Extra};
    use slime_color::LinearColor;
    use slime_reflect::{Kind, Leaf, Problem, Range, Reflect};

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

    /// The numbers of `range` other than `not`, in the order an edit tries them.
    fn picks(range: Range, not: f64) -> Vec<f64> {
        [1.0, 0.5, 2.0, 0.25, range.min + 1.0, range.max - 0.125]
            .into_iter()
            .filter(|&candidate| candidate != not && range.contains(candidate))
            .collect()
    }

    /// A value of the type named `ty`, within `range`, for an option that holds none.
    fn sample(registry: &Registry, ty: &str, range: Range) -> Option<Value> {
        let first = || picks(range, f64::NAN).first().copied();
        match &registry.get(ty)?.info().kind {
            Kind::F32 => Some(Value::F32(first()? as f32)),
            Kind::F64 => Some(Value::F64(first()?)),
            Kind::Struct(fields) => fields
                .iter()
                .map(|field| Some((field.name.clone(), sample(registry, &field.ty, Range::ALL)?)))
                .collect::<Option<Vec<_>>>()
                .map(Value::Struct),
            _ => None,
        }
    }

    /// Other values for `leaf`, which holds `value`, of its type and within its field's range, in
    /// the order an edit tries them, until its type accepts one. None for a leaf that holds an
    /// entity, or an empty list.
    fn others(registry: &Registry, leaf: &Leaf, value: &Value) -> Vec<Value> {
        let range = leaf
            .field
            .as_ref()
            .and_then(|f| f.range)
            .unwrap_or(Range::ALL);
        let one = match value {
            Value::Bool(value) => Value::Bool(!value),
            Value::Int(value) => Value::Int(value + 1),
            Value::UInt(value) => Value::UInt(value + 1),
            Value::F32(value) => {
                return picks(range, f64::from(*value))
                    .into_iter()
                    .map(|number| Value::F32(number as f32))
                    .collect();
            }
            Value::F64(value) => return picks(range, *value).into_iter().map(Value::F64).collect(),
            Value::Text(text) => Value::Text(format!("{text}!")),
            Value::Asset(id) => Value::Asset(id + 1),
            Value::Option(Some(_)) => Value::Option(None),
            Value::Option(None) => {
                let Some(Kind::Option(inner)) = registry.get(&leaf.ty).map(|r| &r.info().kind)
                else {
                    return Vec::new();
                };
                match sample(registry, inner, range) {
                    Some(inner) => Value::Option(Some(Box::new(inner))),
                    None => return Vec::new(),
                }
            }
            Value::Enum(variant, _) => {
                let Some(Kind::Enum(variants)) = registry.get(&leaf.ty).map(|r| &r.info().kind)
                else {
                    return Vec::new();
                };
                match variants
                    .iter()
                    .find(|other| &other.name != variant && other.fields.is_empty())
                {
                    Some(next) => Value::Enum(next.name.clone(), Vec::new()),
                    None => return Vec::new(),
                }
            }
            _ => return Vec::new(),
        };
        vec![one]
    }

    /// Sets each leaf of `entity`'s component of `T` to another value, through its path, and
    /// checks that it reads back, through reflection and typed. A value that the type's check
    /// refuses is followed by the next; a rotation, which is one only whole, is set whole.
    fn every_leaf_is_set_and_read_back<T: Component + Debug>(world: &mut World, entity: Entity) {
        let name = T::type_name();
        let value = world.component_value(entity, &name).expect("held");
        let leaves = world
            .registry()
            .leaves(&name, &value)
            .expect("a value of its type");
        let rotation: Path = "rotation".parse().expect("a path");
        for leaf in leaves {
            let before = world.component_value(entity, &name).expect("held");
            let at = before.get(&leaf.path).expect("a leaf of the value").clone();
            let mut candidates = others(world.registry(), &leaf, &at);
            // Only what no component holds yet is left alone.
            assert!(
                !candidates.is_empty() || leaf.ty == "Entity" || at == Value::List(Vec::new()),
                "{name}.{}: no other value for {at:?}",
                leaf.path
            );
            let mut path = leaf.path.clone();
            if name == "Transform" && leaf.path.to_string().starts_with("rotation.") {
                let refused = world.set_property(entity, &name, &leaf.path, candidates[0].clone());
                assert!(
                    matches!(&refused, Err(WorldError::Refused { error, .. })
                        if matches!(error.problem(), Problem::Invalid(_))),
                    "{name}.{}: {refused:?}",
                    leaf.path
                );
                let held = Quat::from_value(before.get(&rotation).expect("a rotation"));
                candidates = vec![(Quat::from_rotation_y(0.25) * held.expect("one")).to_value()];
                path = rotation.clone();
            }
            let accepted = candidates.into_iter().find(|new| {
                match world.set_property(entity, &name, &path, new.clone()) {
                    Ok(()) => true,
                    Err(WorldError::Refused { error, .. })
                        if matches!(error.problem(), Problem::Invalid(_)) =>
                    {
                        false
                    }
                    Err(error) => panic!("{name}.{path}: {error}"),
                }
            });
            let Some(new) = accepted else {
                assert!(
                    leaf.ty == "Entity" || at == Value::List(Vec::new()),
                    "{name}.{path}"
                );
                continue;
            };
            let after = world.component_value(entity, &name).expect("held");
            assert_eq!(after.get(&path), Some(&new), "{name}.{path}");
            let typed = world.get::<T>(entity).expect("held").to_value();
            assert_eq!(typed, after, "{name}.{path}: the typed component");
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
    fn every_leaf_of_every_component_overridden_through_a_scene_file_reads_back() {
        use crate::SceneLibrary;

        // A prefab of the entity that holds every component, written by a world.
        let mut made = World::new();
        let root = made.spawn();
        let entity = furnished(&mut made);
        made.set_parent(entity, Some(root)).expect("alive");
        let prefab = made.write_scene(root).expect("written").text;
        let (yard, post) = (AssetId::from_u128(1), AssetId::from_u128(2));
        let mut scenes = SceneLibrary::new();
        scenes.insert_file(post, "post", prefab);
        scenes.insert_file(
            yard,
            "yard",
            format!(
                "[assets]\npost = {{ id = \"{post}\" }}\n\n[entities.placed]\nprefab = \"post\"\n"
            ),
        );
        let spawned = |scenes: &mut SceneLibrary| {
            let mut world = World::new();
            world.register::<Lantern>();
            let scene = scenes.resolve(world.registry(), yard).expect("resolved");
            let root = world.spawn_scene("yard", &scene).expect("spawned");
            (world, root)
        };
        let (mut world, root) = spawned(&mut scenes);
        let placement = world.children(root).next().expect("a placement");
        let placed = world.children(placement).next().expect("its entity");
        every_leaf_is_set_and_read_back::<Transform>(&mut world, placed);
        every_leaf_is_set_and_read_back::<Name>(&mut world, placed);
        every_leaf_is_set_and_read_back::<Meshes>(&mut world, placed);
        every_leaf_is_set_and_read_back::<Hidden>(&mut world, placed);
        every_leaf_is_set_and_read_back::<Light>(&mut world, placed);
        every_leaf_is_set_and_read_back::<Extras>(&mut world, placed);
        every_leaf_is_set_and_read_back::<Lantern>(&mut world, placed);
        // Each leaf now holds another value than the prefab's: the file says so, and spawning it
        // gives the same world.
        let written = world.write_scene(root).expect("written");
        assert!(written.left_out.is_empty(), "{:?}", written.left_out);
        scenes.insert_file(yard, "yard", written.text);
        let (reread, _) = spawned(&mut scenes);
        assert_eq!(world.first_difference(&reread), None);
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
        #[derive(Clone, Reflect)]
        #[reflect(name = "test::Saved")]
        struct Saved(u32);
        impl Component for Saved {}
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
    #[test]
    fn a_light_or_a_transform_that_breaks_its_rules_is_refused_whole() {
        let mut world = World::new();
        let entity = furnished(&mut world);
        let path = |text: &str| text.parse::<Path>().expect("a path");
        let refusals = [
            (
                "Light",
                "inner_cone_angle",
                Value::F32(0.9),
                "cones of 0.9 and 0.6",
            ),
            (
                "Light",
                "intensity",
                Value::F32(f32::INFINITY),
                "inf is not a finite number",
            ),
            (
                "Light",
                "color.r",
                Value::F32(-1.0),
                "is negative or not finite",
            ),
            (
                "Light",
                "color.g",
                Value::F32(f32::NAN),
                "is negative or not finite",
            ),
            // Finite, but not once multiplied by the intensity, 30, as the renderer's shader reads
            // it.
            (
                "Light",
                "color.b",
                Value::F32(f32::MAX),
                "its color times its intensity",
            ),
            ("Transform", "rotation.w", Value::F32(0.0), "is not one"),
            (
                "Transform",
                "translation.y",
                Value::F64(f64::INFINITY),
                "is not finite",
            ),
            ("Transform", "scale", Value::F32(f32::NAN), "is not finite"),
        ];
        for (component, at, value, message) in refusals {
            let before = world.component_value(entity, component);
            let error = world
                .set_property(entity, component, &path(at), value)
                .expect_err(message);
            assert!(
                error.to_string().contains(message),
                "{component}.{at}: {error}"
            );
            assert_eq!(world.component_value(entity, component), before);
        }
        // Read back whole, from a value that breaks the rules, a light is refused too.
        let mut light = world.component_value(entity, "Light").expect("held");
        *light.get_mut(&path("outer_cone_angle")).expect("a spot") = Value::F32(0.1);
        assert!(Light::from_value(&light).is_err());
        world
            .set_property(entity, "Light", &path("inner_cone_angle"), Value::F32(0.5))
            .expect("still below the outer cone");
    }

    #[test]
    fn a_component_that_no_living_entity_holds_is_found_by_no_edit() {
        let mut world = World::new();
        let entity = furnished(&mut world);
        let path: Path = "0".parse().expect("a path");
        let other = world.spawn();
        world.insert(other, Name("other".to_owned()));
        world.despawn(other);
        let refusals = [
            (other, "Name", "a dead entity"),
            (entity, "test::Saved", "a type registered but never stored"),
            (entity, "f32", "a type that is no component"),
        ];
        #[derive(Clone, Reflect)]
        #[reflect(name = "test::Saved")]
        struct Saved(u32);
        impl Component for Saved {}
        world.register::<Saved>();
        for (target, component, why) in refusals {
            let error = world
                .set_property(target, component, &path, Value::UInt(1))
                .expect_err(why);
            assert!(
                matches!(error, WorldError::NoComponent { .. }),
                "{why}: {error}"
            );
            assert!(world.component_value(target, component).is_none(), "{why}");
        }
    }
}
