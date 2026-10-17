//! Scenes spawned as entities, and the persistent identities of entities.

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use slime_reflect::{Reflect, Value};

use crate::components::Name;
use crate::entity::Entity;
use crate::error::WorldError;
use crate::identity::{CREATED, PersistentId};
use crate::library::{NO_RECORD, Placement, ResolvedScene};
use crate::scene_file::for_each_entity;
use crate::transform::Transform;
use crate::world::{Identity, SpawnedScene, World, mark_stale};

impl World {
    /// Spawns a resolved scene as entities, under a root entity whose persistent identity is `key`,
    /// and returns the root (`docs/adr/0089-scenes-and-prefabs.md`).
    ///
    /// Each entity of the scene ([`ResolvedScene`]) becomes an entity, child of its parent's, in the
    /// scene's order, with its components. Its persistent identity is `key` followed by its path
    /// from the scene's root, the IDs of the placements it is in then its own: `zone/lamp 3/light`.
    /// The root holds a [`Name`] of `key` unless the scene gives it one. The world remembers each
    /// placement's prefab and overrides ([`World::placement`]), and the scene's name and
    /// fingerprint, which a save records.
    ///
    /// # Errors
    ///
    /// [`WorldError::InvalidKey`] if `key` is empty, holds `/`, or is `created`, which names the
    /// entities created outside scenes; [`WorldError::IdentityInUse`] if an entity has the identity
    /// `key`, or one of the scene's, already, as a scene spawned twice under one key gives;
    /// [`WorldError::UnknownComponent`] if a component's type has no storage in the world, as a
    /// type that the game did not register ([`World::register`]) has;
    /// [`WorldError::RefusedInScene`] if a value breaks a rule of its type's check, which reflection
    /// leaves to the type ([`World::set_component_value`]). Nothing is spawned then.
    ///
    /// # Panics
    ///
    /// If the world runs out of slots ([`World::spawn`]).
    pub fn spawn_scene(&mut self, key: &str, scene: &ResolvedScene) -> Result<Entity, WorldError> {
        if key.is_empty() || key.contains('/') || key == CREATED {
            return Err(WorldError::InvalidKey(key.to_owned()));
        }
        let ids: Vec<PersistentId> = scene
            .records
            .iter()
            .map(|record| {
                PersistentId::new(
                    std::iter::once(key).chain(record.path.iter().map(String::as_str)),
                )
            })
            .collect();
        if let Some(id) = ids.iter().find(|id| self.identities.contains_key(*id)) {
            return Err(WorldError::IdentityInUse(id.clone()));
        }
        let mut storages: HashMap<&str, usize> = HashMap::new();
        for (name, _) in scene.records.iter().flat_map(|record| &record.components) {
            if !storages.contains_key(name.as_str()) {
                let (storage, _) = self.storage_named(name)?;
                storages.insert(name, storage);
            }
        }
        let entities: Vec<Entity> = scene.records.iter().map(|_| self.spawn_bare()).collect();
        for (&entity, id) in entities.iter().zip(ids.iter()) {
            self.identify(entity, id.clone());
        }
        for ((record, &entity), id) in scene.records.iter().zip(&entities).zip(&ids) {
            for (name, value) in &record.components {
                let storage = storages[name.as_str()];
                let inserted = if holds_entity(value) {
                    self.storages[storage]
                        .insert_value(entity.index(), &with_handles(value, &entities))
                } else {
                    self.storages[storage].insert_value(entity.index(), value)
                };
                if let Err(error) = inserted {
                    // Without parents yet: each is despawned alone.
                    for &entity in &entities {
                        self.despawn(entity);
                    }
                    return Err(WorldError::RefusedInScene {
                        scene: scene.name.clone(),
                        entity: id.clone(),
                        component: name.clone(),
                        error: Box::new(error),
                    });
                }
                if self.component_types[storage] == TypeId::of::<Transform>() {
                    mark_stale(&mut self.nodes, &mut self.changes, entity.index());
                    self.teleport(entity);
                }
            }
        }
        if !scene.records[0]
            .components
            .iter()
            .any(|(name, _)| name == "Name")
        {
            self.insert(entities[0], Name(key.to_owned()));
        }
        for (record, &entity) in scene.records.iter().zip(&entities) {
            if let Some(parent) = record.parent {
                self.set_parent(entity, Some(entities[parent as usize]))
                    .expect("both are alive, and a resolved scene is a tree");
            }
            if let Some(placement) = &record.placement {
                self.nodes[entity.index() as usize].placement = Some(Arc::clone(placement));
            }
        }
        self.scenes.insert(
            key.to_owned(),
            SpawnedScene {
                name: scene.name.clone(),
                content: scene.fingerprint,
            },
        );
        Ok(entities[0])
    }

    /// What the world remembers of the placement whose root is `entity`, if it is alive and one: the
    /// prefab placed, and the overrides that the scene that placed it writes for it
    /// (`docs/adr/0089-scenes-and-prefabs.md`).
    pub fn placement(&self, entity: Entity) -> Option<&Placement> {
        if !self.contains(entity) {
            return None;
        }
        self.nodes[entity.index() as usize].placement.as_deref()
    }

    /// The entity whose persistent identity is `id`, if one is alive.
    pub fn find(&self, id: &PersistentId) -> Option<Entity> {
        match id.created_number() {
            Some(number) => self.created.find(number, &self.entities),
            None => self.identities.get(id).copied(),
        }
    }

    /// The entity's persistent identity, if it is alive and has one: a scene's entity, or one created
    /// outside a scene ([`World::spawn`]).
    pub fn persistent_id(&self, entity: Entity) -> Option<PersistentId> {
        if !self.contains(entity) {
            return None;
        }
        match &self.nodes[entity.index() as usize].identity {
            Identity::None => None,
            Identity::Scene(id) => Some(id.clone()),
            Identity::Created(number) => Some(PersistentId::created(*number)),
        }
    }

    fn identify(&mut self, entity: Entity, id: PersistentId) {
        self.identities.insert(id.clone(), entity);
        self.nodes[entity.index() as usize].identity = Identity::Scene(id);
    }
}

/// Whether `value` holds a reference to an entity.
fn holds_entity(value: &Value) -> bool {
    match value {
        Value::Entity(_) => true,
        Value::Struct(fields) | Value::Enum(_, fields) => {
            fields.iter().any(|(_, field)| holds_entity(field))
        }
        Value::Option(Some(held)) => holds_entity(held),
        Value::List(elements) => elements.iter().any(holds_entity),
        _ => false,
    }
}

/// `value`, a resolved scene's, its references to the scene's records made references to the
/// entities spawned for them, `entities`, in the records' order.
fn with_handles(value: &Value, entities: &[Entity]) -> Value {
    let mut value = value.clone();
    for_each_entity(&mut value, &mut |bits| {
        let entity = if *bits == NO_RECORD {
            Entity::NONE
        } else {
            entities[*bits as usize]
        };
        if let Value::Entity(handle) = entity.to_value() {
            *bits = handle;
        }
    });
    value
}

#[cfg(test)]
mod tests {
    use glam::{DAffine3, DVec3, Mat4, Quat, Vec3};
    use slime_assets::{AssetId, CookedScene, Extra, SceneNode};

    use super::*;
    use crate::{Extras, Meshes};

    /// `scene` resolved, as a game resolves a glTF scene that it spawns alone.
    fn resolved(scene: &CookedScene) -> ResolvedScene {
        ResolvedScene::from_cooked(AssetId::from_u128(1), scene)
    }

    fn node(name: &str, parent: Option<u32>, matrix: Mat4) -> SceneNode {
        SceneNode {
            name: name.to_owned(),
            parent,
            transform: matrix.to_cols_array(),
            meshes: Vec::new(),
            light: None,
            extras: Vec::new(),
        }
    }

    /// A chapel turned and moved, its door a child, and a stretched wall beside it.
    fn chapel() -> CookedScene {
        let turned =
            Mat4::from_rotation_translation(Quat::from_rotation_y(0.5), Vec3::new(3.0, 0.0, -40.0));
        let mut door = node(
            "door",
            Some(0),
            Mat4::from_translation(Vec3::new(0.0, 0.0, 10.5)),
        );
        door.meshes = vec![AssetId::from_u128(7)];
        door.extras = vec![("marker".to_owned(), Extra::Text("door".to_owned()))];
        let wall = node(
            "wall",
            None,
            Mat4::from_scale_rotation_translation(
                Vec3::new(4.0, 2.0, 0.5),
                Quat::IDENTITY,
                Vec3::new(10.0, 0.0, 0.0),
            ),
        );
        CookedScene {
            name: "chapel".to_owned(),
            nodes: vec![node("chapel", None, turned), door, wall],
        }
    }

    /// Where the world places each node's meshes, against where the file puts them.
    fn assert_placed_as_in_the_file(world: &World, key: &str, scene: &CookedScene) {
        for (node, expected) in scene.nodes.iter().zip(scene.world_transforms()) {
            let entity = world
                .find(&PersistentId::new([key, node.name.as_str()]))
                .expect("spawned");
            let transform = world.get::<Transform>(entity).expect("placed");
            let placed = world
                .global_transform(entity)
                .expect("placed")
                .to_affine(transform.stretch);
            let expected = DAffine3::from_mat4(Mat4::from_cols_array(&expected).as_dmat4());
            for (a, b) in placed.to_cols_array().iter().zip(expected.to_cols_array()) {
                assert!(
                    (a - b).abs() < 1e-4,
                    "{}: {placed:?} against {expected:?}",
                    node.name
                );
            }
        }
    }

    #[test]
    fn a_cooked_scene_becomes_entities_placed_where_its_file_puts_them() {
        let mut world = World::new();
        let scene = chapel();
        let root = world
            .spawn_scene("zone", &resolved(&scene))
            .expect("spawned");
        assert_eq!(world.len(), 4);
        assert_eq!(world.persistent_id(root), Some(PersistentId::new(["zone"])));
        let door = world
            .find(&PersistentId::new(["zone", "door"]))
            .expect("spawned");
        let chapel = world.parent(door).expect("a child");
        assert_eq!(world.parent(chapel), Some(root));
        assert_eq!(
            world.get::<Meshes>(door),
            Some(&Meshes {
                ids: vec![AssetId::from_u128(7)],
                casts_shadows: true
            })
        );
        assert_eq!(
            world
                .get::<Extras>(door)
                .and_then(|extras| extras.get("marker")),
            Some(&Extra::Text("door".to_owned()))
        );
        assert_eq!(world.get::<Name>(door), Some(&Name("door".to_owned())));
        let wall = world
            .find(&PersistentId::new(["zone", "wall"]))
            .expect("spawned");
        let stretch = world.get::<Transform>(wall).expect("placed").stretch;
        assert!((stretch - Vec3::new(4.0, 2.0, 0.5)).length() < 1e-5);
        assert_placed_as_in_the_file(&world, "zone", &scene);
        world.update_transforms();
        assert_placed_as_in_the_file(&world, "zone", &scene);
    }

    #[test]
    fn identities_follow_names_when_an_export_reorders_the_nodes() {
        let scene = chapel();
        // The wall first, then the chapel and its door.
        let mut reordered = scene.clone();
        reordered.nodes = vec![
            scene.nodes[2].clone(),
            scene.nodes[0].clone(),
            SceneNode {
                parent: Some(1),
                ..scene.nodes[1].clone()
            },
        ];
        let mut first = World::new();
        let mut second = World::new();
        first
            .spawn_scene("zone", &resolved(&scene))
            .expect("spawned");
        second
            .spawn_scene("zone", &resolved(&reordered))
            .expect("spawned");
        for name in ["chapel", "door", "wall"] {
            let id = PersistentId::new(["zone", name]);
            let place = |world: &World| {
                world
                    .global_transform(world.find(&id).expect("spawned"))
                    .expect("placed")
                    .translation
            };
            assert!((place(&first) - place(&second)).length() < 1e-9, "{name}");
        }
    }

    #[test]
    fn a_node_without_a_name_of_its_own_is_known_by_its_index() {
        let mut scene = chapel();
        scene.nodes[2].name = "door".to_owned();
        let mut world = World::new();
        world
            .spawn_scene("zone", &resolved(&scene))
            .expect("spawned");
        assert!(world.find(&PersistentId::new(["zone", "door"])).is_none());
        assert!(world.find(&PersistentId::new(["zone", "#1"])).is_some());
        assert!(world.find(&PersistentId::new(["zone", "#2"])).is_some());
    }

    #[test]
    fn a_name_that_looks_like_an_index_is_known_by_its_own_index() {
        let mut scene = chapel();
        scene.nodes[0].name = "#2".to_owned();
        scene.nodes[2].name = String::new();
        let mut world = World::new();
        world
            .spawn_scene("zone", &resolved(&scene))
            .expect("spawned");
        let named = world
            .find(&PersistentId::new(["zone", "#0"]))
            .expect("spawned");
        assert_eq!(world.get::<Name>(named), Some(&Name("#2".to_owned())));
        let unnamed = world
            .find(&PersistentId::new(["zone", "#2"]))
            .expect("spawned");
        assert_eq!(world.get::<Name>(unnamed), None);
        world.despawn(unnamed);
        assert_eq!(world.find(&PersistentId::new(["zone", "#0"])), Some(named));
    }

    #[test]
    fn a_scene_with_a_node_before_its_parent_panics_before_it_spawns_anything() {
        let mut scene = chapel();
        scene.nodes[0].parent = Some(1);
        let mut world = World::new();
        let spawned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            world.spawn_scene("zone", &resolved(&scene))
        }));
        assert!(spawned.is_err());
        assert!(world.is_empty());
    }

    #[test]
    fn a_scene_spawned_twice_under_one_key_is_refused_and_spawns_nothing() {
        let mut world = World::new();
        world
            .spawn_scene("zone", &resolved(&chapel()))
            .expect("spawned");
        assert_eq!(
            world.spawn_scene("zone", &resolved(&chapel())),
            Err(WorldError::IdentityInUse(PersistentId::new(["zone"])))
        );
        assert_eq!(world.len(), 4);
        world
            .spawn_scene("copy", &resolved(&chapel()))
            .expect("another key");
        let root = world.find(&PersistentId::new(["zone"])).expect("spawned");
        assert!(world.despawn(root));
        assert!(world.find(&PersistentId::new(["zone", "door"])).is_none());
        world
            .spawn_scene("zone", &resolved(&chapel()))
            .expect("the key is free again");
    }

    #[test]
    fn the_children_of_a_node_scaled_unevenly_are_placed_where_the_file_puts_them() {
        let parent = node(
            "stretched",
            None,
            Mat4::from_scale_rotation_translation(
                Vec3::new(2.0, 1.0, 1.0),
                Quat::IDENTITY,
                Vec3::new(1.0, 0.0, 0.0),
            ),
        );
        // Not turned, so the parent's scale gives no shear and the child keeps its exact place.
        let child = node(
            "child",
            Some(0),
            Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)),
        );
        let scene = CookedScene {
            name: "stretched".to_owned(),
            nodes: vec![parent, child],
        };
        let mut world = World::new();
        world
            .spawn_scene("zone", &resolved(&scene))
            .expect("spawned");
        assert_placed_as_in_the_file(&world, "zone", &scene);
        let child = world
            .find(&PersistentId::new(["zone", "child"]))
            .expect("spawned");
        let translation = world.global_transform(child).expect("placed").translation;
        assert!((translation - DVec3::new(3.0, 0.0, 0.0)).length() < 1e-9);
    }
}
