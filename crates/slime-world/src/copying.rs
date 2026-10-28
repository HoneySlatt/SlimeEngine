//! The commands that create again what a world holds, which an editor copies, pastes and
//! duplicates (`docs/adr/0162-creating-copying-and-arranging.md`).

use std::collections::HashSet;

use slime_reflect::Value;

use crate::command::Command;
use crate::entity::Entity;
use crate::error::WorldError;
use crate::identity::PersistentId;
use crate::scene_file::for_each_entity;
use crate::world::World;
use crate::write_back::expected;

/// What [`World::creating_commands`] gathers: the commands that make the entities, in the order of
/// the hierarchy, then those that insert the components to insert once every entity is made.
#[derive(Default)]
struct Gathered {
    made: Vec<Command>,
    inserted: Vec<Command>,
}

impl World {
    /// The commands that create again the subtrees of `roots`, as they are, each under the parent
    /// that its root has, last among its children (`docs/adr/0162-creating-copying-and-arranging.md`):
    /// a placement as [`Command::Place`], then [`Command::Delete`] for each entity of its prefab
    /// that it lost and whose parent in the prefab it kept, which takes what is below it, and
    /// [`Command::Remove`] for each component that one lost; any other entity as
    /// [`Command::Create`], with its components; the entities below each, in the order of the
    /// hierarchy. Then [`Command::Insert`] for each component of a placement's entities whose value
    /// differs from its prefab's, or that its prefab does not give, and for each component of
    /// another entity that refers to an entity of the subtrees: inserted once every entity is made,
    /// so that the commands, read in their order, find what they refer to. An entity of a
    /// placement whose parent in the prefab the placement lost, as one moved out of its place, is
    /// left out, as a scene file loses it. The commands name what they create by the identities of
    /// what they copy, which [`Command::read_renamed`] renames as it reads them. A root below
    /// another, or given twice, is copied once.
    ///
    /// # Errors
    ///
    /// [`WorldError::NotAlive`] for a root that is not alive; [`WorldError::NoIdentity`] for an
    /// entity of the subtrees, or a root's parent, without a persistent identity.
    pub fn creating_commands(&self, roots: &[Entity]) -> Result<Vec<Command>, WorldError> {
        if let Some(&dead) = roots.iter().find(|&&root| !self.contains(root)) {
            return Err(WorldError::NotAlive(dead));
        }
        // The roots that no other root holds, each once, and every entity of their subtrees.
        let mut outermost: Vec<Entity> = Vec::new();
        for &root in roots {
            if !outermost.contains(&root)
                && std::iter::successors(self.parent(root), |&each| self.parent(each))
                    .all(|ancestor| !roots.contains(&ancestor))
            {
                outermost.push(root);
            }
        }
        let mut copied = HashSet::new();
        let mut stack = outermost.clone();
        while let Some(entity) = stack.pop() {
            copied.insert(entity.to_bits());
            stack.extend(self.children(entity));
        }
        let mut gathered = Gathered::default();
        for root in outermost {
            let parent = self
                .parent(root)
                .map(|parent| self.identity_of(parent))
                .transpose()?;
            self.gather(root, parent, &copied, &mut gathered)?;
        }
        let mut commands = gathered.made;
        commands.extend(gathered.inserted);
        Ok(commands)
    }

    /// The persistent identity of `entity`, which a command names it by.
    fn identity_of(&self, entity: Entity) -> Result<PersistentId, WorldError> {
        self.persistent_id(entity)
            .ok_or(WorldError::NoIdentity(entity))
    }

    /// Adds to `gathered` the commands that create `entity`'s subtree again under `parent`, the
    /// entities of `copied` those whose references the components inserted last hold.
    fn gather(
        &self,
        entity: Entity,
        parent: Option<PersistentId>,
        copied: &HashSet<u64>,
        gathered: &mut Gathered,
    ) -> Result<(), WorldError> {
        let id = self.identity_of(entity)?;
        let Some(placement) = self.placement(entity) else {
            let mut components = Vec::new();
            for name in self.component_names(entity) {
                let value = self
                    .component_value(entity, name)
                    .expect("a component that the entity holds");
                if refers_to(&value, copied) {
                    gathered.inserted.push(Command::Insert {
                        entity: id.clone(),
                        component: name.to_owned(),
                        value,
                    });
                } else {
                    components.push((name.to_owned(), value));
                }
            }
            gathered.made.push(Command::Create {
                entity: id.clone(),
                parent,
                rank: None,
                components,
            });
            for child in self.children(entity) {
                self.gather(child, Some(id.clone()), copied, gathered)?;
            }
            return Ok(());
        };
        gathered.made.push(Command::Place {
            entity: id.clone(),
            parent,
            rank: None,
            prefab: placement.prefab().clone(),
        });
        let placed = self.placed(entity, placement);
        let entities: Vec<Option<Entity>> = placed.iter().map(|(_, entity)| *entity).collect();
        let in_prefab: HashSet<Entity> = entities.iter().flatten().copied().collect();
        let root_name = self.file_id(entity);
        // Each record whose entity the copy loses: lost by the placement, or below one lost.
        let mut lost = vec![false; placed.len()];
        for (index, (record, found)) in placed.iter().enumerate() {
            let record_id =
                PersistentId::new(id.parts().iter().chain(&record.path).map(String::as_str));
            let parent_lost = record.parent.is_some_and(|parent| lost[parent as usize]);
            let Some(found) = found.filter(|_| !parent_lost) else {
                // Lost: deleted, unless its parent is lost too, which takes it with it.
                lost[index] = true;
                if !parent_lost {
                    gathered.made.push(Command::Delete { entity: record_id });
                }
                continue;
            };
            // The components that the prefab gives it, its root's name included.
            let given: Vec<&str> = record
                .components
                .iter()
                .map(|(name, _)| name.as_str())
                .chain(
                    (index == 0 && !record.components.iter().any(|(name, _)| name == "Name"))
                        .then_some("Name"),
                )
                .collect();
            for name in &given {
                if self.component_value(found, name).is_none() {
                    gathered.made.push(Command::Remove {
                        entity: record_id.clone(),
                        component: (*name).to_owned(),
                    });
                }
            }
            for name in self.component_names(found) {
                let value = self
                    .component_value(found, name)
                    .expect("a component that the entity holds");
                let stable = self.component_name(name);
                let same = expected(record, index, Some(&root_name), &entities, &stable)
                    .is_some_and(|expected| {
                        matches!(
                            self.registry.difference(&stable, &expected, &value, false),
                            Ok(None)
                        )
                    });
                if !same {
                    gathered.inserted.push(Command::Insert {
                        entity: record_id.clone(),
                        component: name.to_owned(),
                        value,
                    });
                }
            }
            // What the level put below it, beside what its prefab gives.
            for child in self.children(found) {
                if !in_prefab.contains(&child) {
                    self.gather(child, Some(record_id.clone()), copied, gathered)?;
                }
            }
        }
        Ok(())
    }
}

/// Whether `value` refers to an entity of `entities`, by the bits of their handles.
fn refers_to(value: &Value, entities: &HashSet<u64>) -> bool {
    let mut found = false;
    for_each_entity(&mut value.clone(), &mut |bits| {
        found |= entities.contains(bits);
    });
    found
}
