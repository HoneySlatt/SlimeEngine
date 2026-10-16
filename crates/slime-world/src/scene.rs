//! Cooked scenes spawned as entities, and the persistent identities of entities.

use std::collections::HashMap;

use glam::{DAffine3, Mat4};
use slime_assets::CookedScene;

use crate::components::{Extras, Light, Meshes, Name};
use crate::entity::Entity;
use crate::error::WorldError;
use crate::identity::{CREATED, PersistentId};
use crate::transform::{GlobalTransform, Transform, split};
use crate::world::{Identity, SpawnedScene, World};

impl World {
    /// Spawns a cooked scene as entities, under a root entity whose persistent identity is `key`,
    /// and returns the root.
    ///
    /// Each node becomes an entity, child of its parent's, the scene's root nodes children of the
    /// root, in the scene's order. Each holds a [`Transform`] from its node's matrix, its [`Name`]
    /// if it has one, the [`Meshes`] and the [`Light`] it places, and its [`Extras`]. Its
    /// persistent identity is `key` and the node's name, or its index, `#12`, with a warning, when
    /// the name is missing, starts with `#` as indices do, or another node has it too: an export
    /// that reorders the nodes keeps them, a node renamed loses its own. No two entities of a scene
    /// get the same identity.
    ///
    /// A node's matrix gives its transform as `docs/adr/0079-the-engines-own-world.md` says: three
    /// scales within 10⁻⁵ of the largest become its uniform scale; otherwise its stretch, which its
    /// children do not inherit. A node with children scaled unevenly or mirrored is logged with a
    /// warning, and its children are placed from where the file puts them in the world. A shear,
    /// which a node turned under a node scaled unevenly gets, is dropped: its rotation keeps the
    /// direction of its first axis that is not flattened, and the others are made square to it.
    ///
    /// # Errors
    ///
    /// [`WorldError::InvalidKey`] if `key` is empty, holds `/`, or is `created`, which names the
    /// entities created outside scenes; [`WorldError::IdentityInUse`] if an entity has the identity
    /// `key`, or one of the scene's, already, as a scene spawned twice under one key gives. Nothing
    /// is spawned then.
    ///
    /// # Panics
    ///
    /// If a node's parent does not come before it, which a scene read from cooked data never has
    /// (`CookedScene::from_bytes` refuses it), before anything is spawned; or if the world runs out
    /// of slots ([`World::spawn`]).
    pub fn spawn_scene(&mut self, key: &str, scene: &CookedScene) -> Result<Entity, WorldError> {
        let mut has_children = vec![false; scene.nodes.len()];
        for (index, node) in scene.nodes.iter().enumerate() {
            if let Some(parent) = node.parent {
                assert!(
                    (parent as usize) < index,
                    "scene `{}`: node {index}'s parent comes after it",
                    scene.name
                );
                has_children[parent as usize] = true;
            }
        }
        if key.is_empty() || key.contains('/') || key == CREATED {
            return Err(WorldError::InvalidKey(key.to_owned()));
        }
        let root_id = PersistentId::new([key]);
        let locals = local_ids(scene);
        for id in std::iter::once(root_id.clone()).chain(locals.iter().map(|l| root_id.child(l))) {
            if self.identities.contains_key(&id) {
                return Err(WorldError::IdentityInUse(id));
            }
        }
        let root = self.spawn_bare();
        self.insert(root, Name(key.to_owned()));
        self.insert(root, Transform::IDENTITY);
        self.identify(root, root_id.clone());

        // Where the file puts each node in the scene, and where its entity is, which differ below
        // a node with children scaled unevenly.
        let mut exact: Vec<DAffine3> = Vec::with_capacity(scene.nodes.len());
        let mut placed: Vec<GlobalTransform> = Vec::with_capacity(scene.nodes.len());
        let mut flattened: Vec<bool> = Vec::with_capacity(scene.nodes.len());
        let mut entities: Vec<Entity> = Vec::with_capacity(scene.nodes.len());
        for (index, node) in scene.nodes.iter().enumerate() {
            let parent = node.parent.map(|parent| parent as usize);
            let local = DAffine3::from_mat4(Mat4::from_cols_array(&node.transform).as_dmat4());
            let parent_exact = parent.map_or(DAffine3::IDENTITY, |parent| exact[parent]);
            let parent_placed = parent.map_or(GlobalTransform::IDENTITY, |parent| placed[parent]);
            let below_flattened = parent.is_some_and(|parent| flattened[parent]);
            let world = parent_exact * local;
            let matrix = if below_flattened {
                parent_placed.to_affine(glam::Vec3::ONE).inverse() * world
            } else {
                local
            };
            let (transform, flattens) = split(matrix, has_children[index]);
            if flattens {
                tracing::warn!(
                    scene = %scene.name,
                    node = %node.name,
                    "a node with children is scaled unevenly or mirrored: its children are placed apart from it"
                );
            }
            exact.push(world);
            placed.push(parent_placed.mul_transform(&transform));
            flattened.push(flattens || below_flattened);

            let entity = self.spawn_bare();
            self.insert(entity, transform);
            if !node.name.is_empty() {
                self.insert(entity, Name(node.name.clone()));
            }
            if !node.meshes.is_empty() {
                self.insert(
                    entity,
                    Meshes {
                        ids: node.meshes.clone(),
                        casts_shadows: true,
                    },
                );
            }
            if let Some(light) = node.light {
                self.insert(entity, Light::from_cooked(&light));
            }
            if !node.extras.is_empty() {
                self.insert(entity, Extras(node.extras.clone()));
            }
            let parent_entity = parent.map_or(root, |parent| entities[parent]);
            self.set_parent(entity, Some(parent_entity))
                .expect("both are alive, and the parent came before");
            self.identify(entity, root_id.child(&locals[index]));
            entities.push(entity);
        }
        self.scenes.insert(
            key.to_owned(),
            SpawnedScene {
                name: scene.name.clone(),
                content: fingerprint(&scene.to_bytes()),
            },
        );
        Ok(root)
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

/// The FNV-1a hash of `bytes`: a fingerprint of a scene's content that a save keeps, the same on
/// every platform and with every version of Rust.
fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Each node's ID within its scene: its name, or `#<index>` with a warning when the name is
/// missing, starts with `#`, which would let it be taken for another node's index, holds `/`,
/// which separates the parts of an identity, or is repeated. No two nodes get the same ID.
fn local_ids(scene: &CookedScene) -> Vec<String> {
    let mut uses: HashMap<&str, usize> = HashMap::new();
    for node in &scene.nodes {
        *uses.entry(node.name.as_str()).or_default() += 1;
    }
    scene
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            if !node.name.is_empty()
                && !node.name.starts_with('#')
                && !node.name.contains('/')
                && uses[node.name.as_str()] == 1
            {
                node.name.clone()
            } else {
                tracing::warn!(
                    scene = %scene.name,
                    index,
                    name = %node.name,
                    "a node without a name of its own is known by its index, which a new export can change"
                );
                format!("#{index}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use glam::{DVec3, Quat, Vec3};
    use slime_assets::{AssetId, Extra, SceneNode};

    use super::*;

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
        let root = world.spawn_scene("zone", &scene).expect("spawned");
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
        first.spawn_scene("zone", &scene).expect("spawned");
        second.spawn_scene("zone", &reordered).expect("spawned");
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
        world.spawn_scene("zone", &scene).expect("spawned");
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
        world.spawn_scene("zone", &scene).expect("spawned");
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
            world.spawn_scene("zone", &scene)
        }));
        assert!(spawned.is_err());
        assert!(world.is_empty());
    }

    #[test]
    fn a_scene_spawned_twice_under_one_key_is_refused_and_spawns_nothing() {
        let mut world = World::new();
        world.spawn_scene("zone", &chapel()).expect("spawned");
        assert_eq!(
            world.spawn_scene("zone", &chapel()),
            Err(WorldError::IdentityInUse(PersistentId::new(["zone"])))
        );
        assert_eq!(world.len(), 4);
        world.spawn_scene("copy", &chapel()).expect("another key");
        let root = world.find(&PersistentId::new(["zone"])).expect("spawned");
        assert!(world.despawn(root));
        assert!(world.find(&PersistentId::new(["zone", "door"])).is_none());
        world
            .spawn_scene("zone", &chapel())
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
        world.spawn_scene("zone", &scene).expect("spawned");
        assert_placed_as_in_the_file(&world, "zone", &scene);
        let child = world
            .find(&PersistentId::new(["zone", "child"]))
            .expect("spawned");
        let translation = world.global_transform(child).expect("placed").translation;
        assert!((translation - DVec3::new(3.0, 0.0, 0.0)).length() < 1e-9);
    }
}
