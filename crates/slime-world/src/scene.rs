//! Scenes spawned as entities, and the persistent identities of entities.

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use slime_reflect::Value;

use crate::components::Name;
use crate::entity::Entity;
use crate::error::WorldError;
use crate::identity::{CREATED, PersistentId};
use crate::library::{Overrides, Placement, ResolvedScene};
use crate::transform::Transform;
use crate::world::{Identity, SpawnedScene, World, mark_stale};
use crate::write_back::with_handles;

impl World {
    /// Spawns a resolved scene as entities, under a root entity whose persistent identity is `key`,
    /// and returns the root (`docs/adr/0089-scenes-and-prefabs.md`).
    ///
    /// Each entity of the scene ([`ResolvedScene`]) becomes an entity, child of its parent's, in the
    /// scene's order, with its components. Its persistent identity is `key` followed by its path
    /// from the scene's root, the IDs of the placements it is in then its own: `zone/lamp 3/light`.
    /// The root holds a [`Name`] of `key` unless the scene gives it one. The world remembers each
    /// placement's prefab and overrides ([`World::placement`]), and keeps the scene, which a save
    /// names by its name and fingerprint and a file written back reads the table of assets of
    /// ([`World::write_scene`]), even once its root is destroyed, since entities moved out of it live
    /// on, until another is spawned under `key`: a scene that its library holds too costs a count.
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
            .0
            .records
            .iter()
            .map(|record| {
                PersistentId::new(
                    std::iter::once(key).chain(record.path.iter().map(String::as_str)),
                )
            })
            .collect();
        let entities = self.spawn_records(scene, &ids, None)?;
        if !scene.0.records[0]
            .components
            .iter()
            .any(|(name, _)| name == "Name")
        {
            self.insert(entities[0], Name(key.to_owned()));
        }
        self.scenes.insert(
            key.to_owned(),
            SpawnedScene {
                name: scene.0.name.clone(),
                content: scene.0.fingerprint,
                scene: scene.clone(),
            },
        );
        Ok(entities[0])
    }

    /// Spawns `prefab`, a resolved scene, as a placement made while the world runs, a child of
    /// `parent`, or at the world's root, and returns its root (`docs/adr/0089-scenes-and-prefabs.md`).
    ///
    /// The placement's root is an entity created outside a scene: its persistent identity is
    /// `created/<number>`, the next of the world's count ([`World::spawn`]), and its entities'
    /// identities are its own followed by their paths in the prefab, `created/7/light`. The world
    /// remembers its prefab ([`World::placement`]), and writes it as a placement in a scene file
    /// ([`World::write_scene`]). The root holds no [`Name`], unlike a placement that a file
    /// makes, which spawning names by its ID: the world gives it an ID when it first writes it, and
    /// a file written while it has no name says that its `Name` is removed.
    ///
    /// # Errors
    ///
    /// [`WorldError::NotAlive`] if `parent` is not alive; [`WorldError::UnknownComponent`] and
    /// [`WorldError::RefusedInScene`] as [`World::spawn_scene`] says. Nothing is spawned then, and
    /// the count is unchanged.
    ///
    /// # Panics
    ///
    /// If the world runs out of slots ([`World::spawn`]).
    pub fn spawn_placement(
        &mut self,
        prefab: &ResolvedScene,
        parent: Option<Entity>,
    ) -> Result<Entity, WorldError> {
        if let Some(parent) = parent
            && !self.contains(parent)
        {
            return Err(WorldError::NotAlive(parent));
        }
        let number = self.next_created;
        let ids: Vec<PersistentId> = prefab
            .0
            .records
            .iter()
            .map(|record| {
                let number = number.to_string();
                PersistentId::new(
                    [CREATED, number.as_str()]
                        .into_iter()
                        .chain(record.path.iter().map(String::as_str)),
                )
            })
            .collect();
        let entities = self.spawn_records(prefab, &ids, Some(number))?;
        let root = entities[0];
        self.next_created += 1;
        self.created.push(number, root);
        self.nodes[root.index() as usize].placement = Some(Arc::new(Placement {
            prefab: prefab.clone(),
            overrides: Overrides::default(),
            set_aside: Vec::new(),
        }));
        if parent.is_some() {
            self.set_parent(root, parent)
                .expect("both are alive, and the root is new");
        }
        Ok(root)
    }

    /// Spawns the records of `scene`, each under its identity of `ids`, the root under the number
    /// `created` if it is a placement made while the world runs, and returns their entities, in the
    /// records' order: all or nothing.
    fn spawn_records(
        &mut self,
        scene: &ResolvedScene,
        ids: &[PersistentId],
        created: Option<u64>,
    ) -> Result<Vec<Entity>, WorldError> {
        let named = |index: usize| index > 0 || created.is_none();
        if let Some((_, id)) = ids
            .iter()
            .enumerate()
            .find(|&(index, id)| named(index) && self.identities.contains_key(id))
        {
            return Err(WorldError::IdentityInUse(id.clone()));
        }
        let mut storages: HashMap<&str, usize> = HashMap::new();
        for (name, _) in scene.0.records.iter().flat_map(|record| &record.components) {
            if !storages.contains_key(name.as_str()) {
                let (storage, _) = self.storage_named(name)?;
                storages.insert(name, storage);
            }
        }
        let entities: Vec<Entity> = scene.0.records.iter().map(|_| self.spawn_bare()).collect();
        let spawned: Vec<Option<Entity>> = entities.iter().copied().map(Some).collect();
        for (index, (&entity, id)) in entities.iter().zip(ids).enumerate() {
            match created {
                Some(number) if index == 0 => {
                    self.nodes[entity.index() as usize].identity = Identity::Created(number);
                }
                _ => self.identify(entity, id.clone()),
            }
        }
        for ((record, &entity), id) in scene.0.records.iter().zip(&entities).zip(ids) {
            for (name, value) in &record.components {
                let storage = storages[name.as_str()];
                let inserted = if holds_entity(value) {
                    self.storages[storage]
                        .insert_value(entity.index(), &with_handles(value, &spawned))
                } else {
                    self.storages[storage].insert_value(entity.index(), value)
                };
                if let Err(error) = inserted {
                    // Without parents yet: each is despawned alone. The root of a placement made
                    // while the world runs is not in the count yet: it leaves nothing there.
                    if created.is_some() {
                        self.nodes[entities[0].index() as usize].identity = Identity::None;
                    }
                    for &entity in &entities {
                        self.despawn(entity);
                    }
                    return Err(WorldError::RefusedInScene {
                        scene: scene.0.name.clone(),
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
        for (record, &entity) in scene.0.records.iter().zip(&entities) {
            if let Some(parent) = record.parent {
                self.set_parent(entity, Some(entities[parent as usize]))
                    .expect("both are alive, and a resolved scene is a tree");
            }
            if let Some(placement) = &record.placement {
                self.nodes[entity.index() as usize].placement = Some(Arc::clone(placement));
            }
        }
        Ok(entities)
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
