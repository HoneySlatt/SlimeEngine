//! Commands: changes of the world that name their entities by their persistent identities, which
//! the world undoes with the command that applying each gives back, and which files and tools write
//! as text (`docs/adr/0107-the-execution-model.md`, `docs/adr/0109-stable-references-and-undo.md`).

use slime_assets::AssetId;
use slime_reflect::{Path, Problem, ReflectError, Value};
use slime_serialize::{Composer, Document, Mode, ReadError, References};

use crate::deletion::Deleted;
use crate::entity::Entity;
use crate::error::WorldError;
use crate::identity::PersistentId;
use crate::library::{ResolvedScene, SceneError, SceneLibrary};
use crate::world::World;

/// A change of the world asked for outside the phases of a frame that may make it, by the editor,
/// the dev overlay, a tool or a game's presentation, which reads the world without writing it: the
/// host queues it and applies it at the start of the next frame ([`World::apply`],
/// `docs/adr/0107-the-execution-model.md`). It names its entities by their persistent identities,
/// which hold from one run to the next and after a load or a hot reload replaced the world, where a
/// handle would name another entity, or none (`docs/adr/0109-stable-references-and-undo.md`).
///
/// Applying a command gives back the command that undoes it, which the editor keeps to undo and
/// redo. A command but [`Command::Restore`] is written and read as text ([`Command::write`],
/// [`Command::read`]): what the editor's journal holds, and what tools send.
///
/// A rank is a place among a parent's children, the first at 0: a child put at a rank past the
/// last is put last, as `None` puts it. A command that names no parent puts its entity at the
/// world's root.
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
    /// Creates an entity of a scene with its components, as [`World::spawn_as`] does, under
    /// `parent` at `rank`.
    Create {
        /// The new entity's persistent identity, a key under which a scene is spawned and an ID that
        /// the scene's file can hold: `zone/torch-0rdx` ([`World::new_id`]).
        entity: PersistentId,
        /// Its parent, if it has one.
        parent: Option<PersistentId>,
        /// Its rank among its parent's children; `None` for the last.
        rank: Option<usize>,
        /// Its components, each by the name of its type and its value, attached in this order.
        components: Vec<(String, Value)>,
    },
    /// Places a prefab in a scene, as [`World::place_as`] does, under `parent` at `rank`.
    Place {
        /// The placement's root's persistent identity, as for [`Command::Create`].
        entity: PersistentId,
        /// Its parent, if it has one.
        parent: Option<PersistentId>,
        /// Its rank among its parent's children; `None` for the last.
        rank: Option<usize>,
        /// The prefab placed.
        prefab: ResolvedScene,
    },
    /// Deletes an entity and its descendants, as [`World::despawn`] does, and gives back what
    /// creates them again: [`Command::Restore`].
    Delete {
        /// The entity, by its persistent identity.
        entity: PersistentId,
    },
    /// Makes `parent` an entity's parent, the entity at `rank` among its children.
    Reparent {
        /// The entity, by its persistent identity.
        entity: PersistentId,
        /// Its new parent, if it has one.
        parent: Option<PersistentId>,
        /// Its rank among its parent's children; `None` for the last.
        rank: Option<usize>,
    },
    /// Attaches a component, in place of the one of its type that the entity holds, if it holds
    /// one, as [`World::insert_component_value`] does.
    Insert {
        /// The entity, by its persistent identity.
        entity: PersistentId,
        /// The component's type, by its stable name or an alias.
        component: String,
        /// The component.
        value: Value,
    },
    /// Detaches a component, as [`World::remove_component_value`] does.
    Remove {
        /// The entity, by its persistent identity.
        entity: PersistentId,
        /// The component's type, by its stable name or an alias.
        component: String,
    },
    /// Creates again what a deletion took out of the world, under the same identities and at the
    /// same places, and points again at them the references that other entities held, by setting
    /// those entities' components back to their values at the deletion: what applying
    /// [`Command::Delete`] gives back, which only that world restores, and which is exact when
    /// nothing changed those components since, as in the editor's history.
    Restore(Deleted),
}

impl Command {
    /// The name of the command's kind, as its text names it: `SetProperty`, `Create`...
    pub fn kind(&self) -> &'static str {
        match self {
            Self::SetProperty { .. } => "SetProperty",
            Self::Create { .. } => "Create",
            Self::Place { .. } => "Place",
            Self::Delete { .. } => "Delete",
            Self::Reparent { .. } => "Reparent",
            Self::Insert { .. } => "Insert",
            Self::Remove { .. } => "Remove",
            Self::Restore(_) => "Restore",
        }
    }
}

impl World {
    /// Applies `command`, and returns the command that undoes it: what a host does at the start of
    /// each frame with the commands queued since the last one, and what the editor does with its
    /// own.
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
    /// let undo = world.apply(&dim).expect("the lantern is alive, and 12 is in range");
    /// assert!(matches!(world.get::<Light>(lantern), Some(Light::Point { intensity: 12.0, .. })));
    /// world.apply(&undo).expect("undone");
    /// assert!(matches!(world.get::<Light>(lantern), Some(Light::Point { intensity: 30.0, .. })));
    /// ```
    ///
    /// The command that undoes [`Command::Delete`] holds a copy of what it deleted
    /// ([`Command::Restore`]): recording it reads each component of the subtree, and each component
    /// of the world of a type that may refer to an entity, to find what refers to the subtree.
    /// Measured in a release build on the owner's machine (`slime-benches`'s `world.undo.1k`): a
    /// subtree of 1 000 entities, each with a name, a transform and a light, in a level of 100 000,
    /// is deleted in 1.4 ms and restored in 1.8 ms.
    ///
    /// # Errors
    ///
    /// [`WorldError::NotFound`] when no entity alive has a persistent identity that the command
    /// names, and otherwise the errors of the method that it names. The world holds what it held
    /// then: a refused `Create` or `Restore` destroys what it spawned, which only the world's ticks
    /// and its log of what was destroyed tell ([`World::structure`], [`World::destroyed`]).
    pub fn apply(&mut self, command: &Command) -> Result<Command, WorldError> {
        match command {
            Command::SetProperty {
                entity,
                component,
                path,
                value,
            } => {
                let found = self.found(entity)?;
                let current = self.component_value(found, component).ok_or_else(|| {
                    WorldError::NoComponent {
                        entity: found,
                        component: component.clone(),
                    }
                })?;
                // A path that names no part is refused as setting it would be.
                let before = current
                    .get(path)
                    .cloned()
                    .ok_or_else(|| WorldError::Refused {
                        component: self.component_name(component),
                        error: ReflectError::at(path.clone(), Problem::NoSuchPath),
                    })?;
                self.set_property(found, component, path, value.clone())?;
                Ok(Command::SetProperty {
                    entity: entity.clone(),
                    component: self.component_name(component),
                    path: path.clone(),
                    value: before,
                })
            }
            Command::Create {
                entity,
                parent,
                rank,
                components,
            } => {
                let parent = self.found_parent(parent.as_ref())?;
                let created = self.spawn_as(entity)?;
                for (name, value) in components {
                    if let Err(error) = self.insert_component_value(created, name, value.clone()) {
                        self.despawn(created);
                        return Err(error);
                    }
                }
                self.put_under(created, parent, *rank)?;
                Ok(Command::Delete {
                    entity: entity.clone(),
                })
            }
            Command::Place {
                entity,
                parent,
                rank,
                prefab,
            } => {
                let parent = self.found_parent(parent.as_ref())?;
                let placed = self.place_as(entity, prefab)?;
                self.put_under(placed, parent, *rank)?;
                Ok(Command::Delete {
                    entity: entity.clone(),
                })
            }
            Command::Delete { entity } => {
                let found = self.found(entity)?;
                let deleted = self.record_deletion(entity, found);
                self.despawn(found);
                Ok(Command::Restore(deleted))
            }
            Command::Reparent {
                entity,
                parent,
                rank,
            } => {
                let found = self.found(entity)?;
                let parent = self.found_parent(parent.as_ref())?;
                let before = self
                    .parent(found)
                    .and_then(|parent| self.persistent_id(parent));
                let rank_before = self.rank(found);
                self.put_under(found, parent, *rank)?;
                Ok(Command::Reparent {
                    entity: entity.clone(),
                    parent: before,
                    rank: rank_before,
                })
            }
            Command::Insert {
                entity,
                component,
                value,
            } => {
                let found = self.found(entity)?;
                let before = self.component_value(found, component);
                self.insert_component_value(found, component, value.clone())?;
                let (entity, component) = (entity.clone(), self.component_name(component));
                Ok(match before {
                    Some(value) => Command::Insert {
                        entity,
                        component,
                        value,
                    },
                    None => Command::Remove { entity, component },
                })
            }
            Command::Remove { entity, component } => {
                let found = self.found(entity)?;
                let value = self
                    .remove_component_value(found, component)
                    .ok_or_else(|| WorldError::NoComponent {
                        entity: found,
                        component: component.clone(),
                    })?;
                Ok(Command::Insert {
                    entity: entity.clone(),
                    component: self.component_name(component),
                    value,
                })
            }
            Command::Restore(deleted) => {
                self.restore(deleted)?;
                Ok(Command::Delete {
                    entity: deleted.root().clone(),
                })
            }
        }
    }

    /// The entity alive whose persistent identity is `id`.
    fn found(&self, id: &PersistentId) -> Result<Entity, WorldError> {
        self.find(id)
            .ok_or_else(|| WorldError::NotFound(id.clone()))
    }

    /// The entity alive whose persistent identity is `id`, if `id` names one.
    fn found_parent(&self, id: Option<&PersistentId>) -> Result<Option<Entity>, WorldError> {
        id.map(|id| self.found(id)).transpose()
    }

    /// The stable name of the component type named `component`, by its name or an alias.
    fn component_name(&self, component: &str) -> String {
        self.registry
            .get(component)
            .map_or_else(|| component.to_owned(), |ty| ty.info().name.clone())
    }

    /// Puts `entity` under `parent` at `rank`, or at the world's root.
    fn put_under(
        &mut self,
        entity: Entity,
        parent: Option<Entity>,
        rank: Option<usize>,
    ) -> Result<(), WorldError> {
        match (parent, rank) {
            (Some(parent), Some(rank)) => self.set_parent_at(entity, parent, rank),
            (parent, _) => self.set_parent(entity, parent),
        }
    }
}

/// The keys of each kind of command that its text holds, the kind first.
const KINDS: [(&str, &[&str]); 7] = [
    ("SetProperty", &["entity", "component", "path", "value"]),
    ("Create", &["entity", "parent", "rank", "components"]),
    ("Place", &["entity", "parent", "rank", "prefab"]),
    ("Delete", &["entity"]),
    ("Reparent", &["entity", "parent", "rank"]),
    ("Insert", &["entity", "component", "value"]),
    ("Remove", &["entity", "component"]),
];

impl Command {
    /// The command as text, a document of the engine's notation (`slime_serialize::format`), that
    /// [`Command::read`] reads back in the same world: a table named after its kind, whose keys
    /// are its fields; entities, those that the command names and those that its values refer to,
    /// by their persistent identities, and a prefab by its asset ID. A rank that is `None`, a
    /// parent that is `None` and an empty path are left out.
    ///
    /// ```
    /// use slime_reflect::Value;
    /// use slime_world::{Command, Name, World};
    ///
    /// let mut world = World::new();
    /// let well = world.spawn_keyed("well").unwrap();
    /// world.insert(well, Name("well".to_owned()));
    /// let rename = Command::SetProperty {
    ///     entity: "well".parse().unwrap(),
    ///     component: "Name".to_owned(),
    ///     path: "0".parse().unwrap(),
    ///     value: Value::Text("old well".to_owned()),
    /// };
    /// let text = rename.write(&world).expect("the well is alive");
    /// assert_eq!(
    ///     text,
    ///     "SetProperty = { entity = \"well\", component = \"Name\", path = \"0\", value = \"old well\" }\n"
    /// );
    /// let mut scenes = slime_world::SceneLibrary::new();
    /// assert_eq!(Command::read(&text, &world, &mut scenes).expect("read back"), rename);
    /// ```
    ///
    /// # Errors
    ///
    /// [`CommandTextError::Restore`] for [`Command::Restore`]; [`CommandTextError::World`] for a
    /// [`Command::SetProperty`] whose entity is not alive or holds no such component, whose part's
    /// type the text needs; [`CommandTextError::Write`] for a value that cannot be written, as
    /// `slime-serialize` says.
    pub fn write(&self, world: &World) -> Result<String, CommandTextError> {
        let text = |text: &dyn ToString| Value::Text(text.to_string());
        let place = |parent: &Option<PersistentId>, rank: &Option<usize>| {
            let parent = parent
                .as_ref()
                .map(|parent| ("parent", "String".to_owned(), text(parent)));
            let rank = rank.map(|rank| ("rank", "u64".to_owned(), Value::UInt(rank as u64)));
            parent.into_iter().chain(rank)
        };
        let entity = |entity: &PersistentId| ("entity", "String".to_owned(), text(entity));
        let component = |component: &str| ("component", "String".to_owned(), text(&component));
        // Each key with the type of its value, in the table's order.
        let mut keys: Vec<(&str, String, Value)> = Vec::new();
        let mut components: &[(String, Value)] = &[];
        match self {
            Self::SetProperty {
                entity: id,
                component: name,
                path,
                value,
            } => {
                let ty = part_type(world, id, name, path)?;
                keys.extend([entity(id), component(name)]);
                if !path.is_empty() {
                    keys.push(("path", "String".to_owned(), text(path)));
                }
                keys.push(("value", ty, value.clone()));
            }
            Self::Create {
                entity: id,
                parent,
                rank,
                components: held,
            } => {
                keys.push(entity(id));
                keys.extend(place(parent, rank));
                components = held;
            }
            Self::Place {
                entity: id,
                parent,
                rank,
                prefab,
            } => {
                keys.push(entity(id));
                keys.extend(place(parent, rank));
                let prefab = Value::Asset(prefab.id().as_u128());
                keys.push(("prefab", "AssetId".to_owned(), prefab));
            }
            Self::Delete { entity: id } => keys.push(entity(id)),
            Self::Reparent {
                entity: id,
                parent,
                rank,
            } => {
                keys.push(entity(id));
                keys.extend(place(parent, rank));
            }
            Self::Insert {
                entity: id,
                component: name,
                value,
            } => {
                keys.extend([entity(id), component(name)]);
                keys.push(("value", name.clone(), value.clone()));
            }
            Self::Remove {
                entity: id,
                component: name,
            } => keys.extend([entity(id), component(name)]),
            Self::Restore(_) => return Err(CommandTextError::Restore),
        }
        let kind = self.kind();
        let names = Identities(world);
        let mut composer = Composer::new(world.registry()).naming(&names);
        for (key, ty, value) in &keys {
            composer
                .put(&[kind, key], ty, value)
                .map_err(CommandTextError::Write)?;
        }
        for (name, value) in components {
            composer
                .put(&[kind, "components", name], name, value)
                .map_err(CommandTextError::Write)?;
        }
        composer.finish().map_err(CommandTextError::Write)
    }

    /// The command that `text` writes, as [`Command::write`] writes it, read strictly in `world`,
    /// as it is when the command is about to be applied: the entities that its values refer to
    /// are found there, and so is the part that a [`Command::SetProperty`] sets, whose type its
    /// value is read as; a [`Command::Place`]'s prefab is resolved through `scenes`. An entity
    /// that a value names `""` is no entity.
    ///
    /// # Errors
    ///
    /// [`CommandTextError::Read`] for text that is not a document of the notation, or a value
    /// that its type refuses, a reference to an entity that `world` does not have among them;
    /// [`CommandTextError::Invalid`] for a document that is not a command: not one table, of a
    /// kind that is not one of [`Command`]'s, a key missing or unknown, an identity or a path that
    /// cannot be one; [`CommandTextError::World`] for a [`Command::SetProperty`] whose entity is
    /// not alive or holds no such component; [`CommandTextError::Prefab`] for a prefab that
    /// `scenes` cannot resolve.
    pub fn read(
        text: &str,
        world: &World,
        scenes: &mut SceneLibrary,
    ) -> Result<Self, CommandTextError> {
        let registry = world.registry();
        let names = Identities(world);
        let mut document =
            Document::parse(registry, text, Mode::Strict).map_err(CommandTextError::Read)?;
        let kinds = || {
            KINDS
                .iter()
                .map(|(kind, _)| format!("`{kind}`"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let (kind, allowed) = match document.keys(&[]).as_slice() {
            [kind] if document.is_table(&[kind]) => KINDS
                .iter()
                .find(|(name, _)| name == kind)
                .copied()
                .ok_or_else(|| {
                    CommandTextError::Invalid(format!(
                        "`{kind}` is not a kind of command: {}",
                        kinds()
                    ))
                })?,
            _ => {
                return Err(CommandTextError::Invalid(format!(
                    "a command is one table, named after its kind: {}",
                    kinds()
                )));
            }
        };
        if let Some(key) = document
            .keys(&[kind])
            .into_iter()
            .find(|key| !allowed.contains(&key.as_str()))
        {
            return Err(CommandTextError::Invalid(format!(
                "`{kind}` has no key `{key}`: its keys are {}",
                allowed.join(", ")
            )));
        }
        let mut reader = Reader {
            document: &mut document,
            names: &names,
            kind,
        };
        let entity = reader
            .identity("entity")?
            .ok_or_else(|| reader.missing("entity"))?;
        Ok(match kind {
            "SetProperty" => {
                let component = reader
                    .text("component")?
                    .ok_or_else(|| reader.missing("component"))?;
                let path: Path = reader
                    .text("path")?
                    .unwrap_or_default()
                    .parse()
                    .map_err(|error| CommandTextError::Invalid(format!("{error}")))?;
                let ty = part_type(world, &entity, &component, &path)?;
                let value = reader
                    .value("value", &ty)?
                    .ok_or_else(|| reader.missing("value"))?;
                Self::SetProperty {
                    entity,
                    component,
                    path,
                    value,
                }
            }
            "Create" => {
                let parent = reader.identity("parent")?;
                let rank = reader.rank()?;
                if reader
                    .document
                    .keys(&[kind])
                    .iter()
                    .any(|key| key == "components")
                    && !reader.document.is_table(&[kind, "components"])
                {
                    return Err(CommandTextError::Invalid(format!(
                        "`{kind}`'s `components` is a table of components by their types' names"
                    )));
                }
                let mut components = Vec::new();
                for name in reader.document.keys(&[kind, "components"]) {
                    let value = reader
                        .document
                        .read(&[kind, "components", &name], &name, Some(&names))
                        .map_err(CommandTextError::Read)?;
                    components.extend(value.map(|value| (name, value)));
                }
                Self::Create {
                    entity,
                    parent,
                    rank,
                    components,
                }
            }
            "Place" => {
                let parent = reader.identity("parent")?;
                let rank = reader.rank()?;
                let Some(Value::Asset(id)) = reader.value("prefab", "AssetId")? else {
                    return Err(reader.missing("prefab"));
                };
                let prefab = scenes
                    .resolve(registry, AssetId::from_u128(id))
                    .map_err(CommandTextError::Prefab)?;
                Self::Place {
                    entity,
                    parent,
                    rank,
                    prefab,
                }
            }
            "Delete" => Self::Delete { entity },
            "Reparent" => Self::Reparent {
                entity,
                parent: reader.identity("parent")?,
                rank: reader.rank()?,
            },
            "Insert" => {
                let component = reader
                    .text("component")?
                    .ok_or_else(|| reader.missing("component"))?;
                let value = reader
                    .value("value", &component)?
                    .ok_or_else(|| reader.missing("value"))?;
                Self::Insert {
                    entity,
                    component,
                    value,
                }
            }
            _ => Self::Remove {
                entity,
                component: reader
                    .text("component")?
                    .ok_or_else(|| reader.missing("component"))?,
            },
        })
    }
}

/// The name of the type of the part of `entity`'s component `component` that `path` leads to, as
/// the world holds it now.
fn part_type(
    world: &World,
    entity: &PersistentId,
    component: &str,
    path: &Path,
) -> Result<String, CommandTextError> {
    let found = world.found(entity)?;
    let current =
        world
            .component_value(found, component)
            .ok_or_else(|| WorldError::NoComponent {
                entity: found,
                component: component.to_owned(),
            })?;
    world
        .registry()
        .type_at(&world.component_name(component), &current, path)
        .map_err(|error| {
            CommandTextError::World(WorldError::Refused {
                component: component.to_owned(),
                error,
            })
        })
}

/// Reads the keys of a command's table.
struct Reader<'d, 'a> {
    document: &'d mut Document<'a>,
    names: &'d Identities<'a>,
    kind: &'static str,
}

impl Reader<'_, '_> {
    /// The value of the type named `ty` at `key`, if the table holds one.
    fn value(&mut self, key: &str, ty: &str) -> Result<Option<Value>, CommandTextError> {
        self.document
            .read(&[self.kind, key], ty, Some(self.names))
            .map_err(CommandTextError::Read)
    }

    /// The text at `key`, if the table holds one.
    fn text(&mut self, key: &str) -> Result<Option<String>, CommandTextError> {
        Ok(match self.value(key, "String")? {
            Some(Value::Text(text)) => Some(text),
            _ => None,
        })
    }

    /// The persistent identity at `key`, if the table holds one.
    fn identity(&mut self, key: &str) -> Result<Option<PersistentId>, CommandTextError> {
        self.text(key)?
            .map(|text| {
                text.parse()
                    .map_err(|error| CommandTextError::Invalid(format!("`{key}`: {error}")))
            })
            .transpose()
    }

    /// The rank, if the table holds one.
    fn rank(&mut self) -> Result<Option<usize>, CommandTextError> {
        Ok(match self.value("rank", "u64")? {
            Some(Value::UInt(rank)) => Some(usize::try_from(rank).unwrap_or(usize::MAX)),
            _ => None,
        })
    }

    /// The error of a table without `key`.
    fn missing(&self, key: &str) -> CommandTextError {
        CommandTextError::Invalid(format!("`{}` has no `{key}`", self.kind))
    }
}

/// Names a world's entities by their persistent identities, as its commands and the development
/// channel write them (`docs/adr/0122-the-development-channel.md`): an entity without one, or no
/// entity, is `""`, which reads as no entity.
pub struct Identities<'w>(pub &'w World);

impl References for Identities<'_> {
    fn name(&self, entity: u64) -> Option<String> {
        let id = Entity::from_bits(entity)
            .filter(|&entity| self.0.contains(entity))
            .and_then(|entity| self.0.persistent_id(entity));
        Some(id.map_or_else(String::new, |id| id.to_string()))
    }

    /// `None`, which a reading refuses, for an identity that no entity alive has.
    fn entity(&self, name: &str) -> Option<u64> {
        if name.is_empty() {
            return Some(Entity::NONE.to_bits());
        }
        Some(self.0.find(&name.parse().ok()?)?.to_bits())
    }
}

/// Why a command cannot be written as text, or text read as a command ([`Command::write`],
/// [`Command::read`]).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CommandTextError {
    /// A value cannot be written, as `slime-serialize` says.
    #[error("the command cannot be written: {0}")]
    Write(#[source] ReflectError),
    /// A restoration holds what a deletion took out of one world, which no text holds.
    #[error(
        "a restoration cannot be written: only the world that deleted what it holds restores it"
    )]
    Restore,
    /// The text is not a document of the notation, or holds a value that its type refuses.
    #[error("the command cannot be read: {0}")]
    Read(#[source] ReadError),
    /// The document is not a command, as the text says.
    #[error("{0}")]
    Invalid(String),
    /// The world does not hold what the command's text needs.
    #[error(transparent)]
    World(#[from] WorldError),
    /// The prefab cannot be resolved.
    #[error("the prefab cannot be resolved: {0}")]
    Prefab(#[source] SceneError),
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
        let undo = world.apply(&dim(&id, 5.0)).expect("applied");
        assert!(matches!(
            world.get::<Light>(lantern),
            Some(Light::Point { intensity: 5.0, .. })
        ));
        assert_eq!(undo, dim(&id, 30.0));
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
