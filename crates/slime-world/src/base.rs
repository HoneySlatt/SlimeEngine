//! The base of a world's saves: what the game made of the world at the start of a run, and of each
//! scene that it spawned since, kept as the run changes it (`docs/adr/0125-saves-per-scene.md`).

use std::collections::BTreeSet;

use crate::change::Tick;
use crate::entity::Entity;
use crate::identity::PersistentId;
use crate::world::{Identity, World};

impl World {
    /// The world as the game prepared it: what it holds now is the base that its saves compare it
    /// with (`docs/adr/0125-saves-per-scene.md`). A game settles its world once it has made the
    /// start of a run, and again once it has prepared a scene that it spawned under a key while the
    /// run goes on, a level that the run changed to, as it adds its own components to its entities.
    /// A game's preparation changes the scene's entities only: what it creates, under a key or as a
    /// placement, is the run's, which a save holds, and which loading the save creates before it
    /// prepares the scenes that it spawns ([`World::load`]).
    ///
    /// The first time, it settles every entity that has an identity: the scenes spawned under keys
    /// ([`World::spawn_scene`]), the entities created under keys ([`World::spawn_keyed`]) and the
    /// placements. An entity that the start created outside scenes without a key, alive then, makes
    /// a save of the world refuse ([`World::save`]) while it is alive and not left out of saves
    /// ([`Unsaved`](crate::Unsaved)): a later start may give its number to another. Later, it
    /// settles the scenes spawned under keys since. What the run creates outside scenes after the
    /// start, under a key or not, a save holds whole; a placement that it makes is settled as it
    /// spawns, its prefab's values its base, which a save writes as the prefab and what differs
    /// from it. A scene is settled already when it is spawned, as it spawns: settling it makes what
    /// the game did to it since part of its base.
    ///
    /// The world keeps the base of a value from its first write after its entity was settled, a
    /// copy of each value changed, added or removed since, and the identity of each entity settled
    /// that the run destroys: what a save writes, so that it costs what changed. What the game
    /// destroyed of a scene that it settles, as it prepared it, the run did not destroy. Settling a
    /// scene spawned since applies what the world kept of it when it unloaded it
    /// ([`World::unload_scene`]), as changes since its base; what cannot be applied, as when the
    /// scene changed since, is logged at the `warn` level, not returned. It walks the world's
    /// entities once.
    pub fn settle(&mut self) {
        let tick = self.next_tick();
        let first = self.settled.is_none();
        // The scenes that it settles: those spawned under keys since, every one the first time.
        let keys: BTreeSet<String> = self
            .scenes
            .iter()
            .filter(|(_, scene)| !scene.settled)
            .map(|(key, _)| key.clone())
            .collect();
        let alive: Vec<Entity> = self.entities().collect();
        let mut settling = Vec::new();
        let mut unkeyed = Vec::new();
        for entity in alive {
            let node = &self.nodes[entity.index() as usize];
            let settles = match &node.identity {
                Identity::None => false,
                Identity::Created(number) => {
                    if first {
                        unkeyed.push(*number);
                    }
                    first && node.placement.is_some()
                }
                Identity::Scene(id) => first || keys.contains(&id.parts()[0]),
            };
            if settles {
                settling.push(entity);
            }
        }
        // What the game destroyed of what it settles now, as it prepared it, the run did not; but
        // what was adopted out of it, which saves remove from it for good.
        if first {
            self.gone.clear();
        } else {
            let (formers, scenes) = (&self.formers, &self.scenes);
            self.gone.retain(|id, _| {
                let key = &id.parts()[0];
                let scene = scenes.get(key).map(|held| held.scene.id());
                !keys.contains(key)
                    || formers.values().any(|former| {
                        Some(former.scene) == scene && id.parts().starts_with(former.id.parts())
                    })
            });
        }
        for &entity in &settling {
            let slot = entity.index();
            // An entity whose parent was adopted out of its scene keeps the parent's former
            // identity as its parent at its base: a save says where it is.
            let parent = self
                .parent(entity)
                .and_then(|parent| self.persistent_id(parent))
                .and_then(|id| self.former_of(&id))
                .filter(|former| {
                    self.persistent_id(entity)
                        .is_some_and(|id| id.parts()[0] == former.parts()[0])
                });
            let node = &mut self.nodes[slot as usize];
            node.base = tick;
            node.parent_kept = parent.map(Some);
            for storage in &mut self.storages {
                storage.forget_kept(slot);
            }
        }
        for key in &keys {
            if let Some(scene) = self.scenes.get_mut(key) {
                scene.settled = true;
            }
        }
        if first {
            self.start_created = self.next_created;
            self.unkeyed_at_start = unkeyed;
        }
        self.settled = Some(tick);
        // What the world kept of them when it unloaded them: changes since their base.
        for key in keys {
            let asset = self.scenes[&key].scene.id();
            let at = (key, asset);
            if let Some(kept) = self.unloaded.remove(&at) {
                self.apply_unloaded(&at.0, at.1, kept);
            }
        }
    }

    /// Settles `entities`, spawned by a scene or a placement: their values as the scene gives them
    /// are their base, until the game settles them again ([`World::settle`]).
    pub(crate) fn base_spawned(&mut self, entities: &[Entity]) {
        let tick: Tick = self.next_tick();
        for entity in entities {
            self.nodes[entity.index() as usize].base = tick;
        }
    }

    /// Forgets what the run destroyed of the scene whose root is `root`, as the scene goes.
    pub(crate) fn forget_gone_under(&mut self, root: &PersistentId) {
        self.gone
            .retain(|id, _| !id.parts().starts_with(root.parts()));
    }
}
