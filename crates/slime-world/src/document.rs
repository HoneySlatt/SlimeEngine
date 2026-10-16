//! The world's documents: a save, what differs between a world and the world that a game makes at
//! the start of a run, and that difference applied again to a world made so
//! (`docs/adr/0087-the-worlds-documents.md`).

use std::collections::BTreeMap;

use slime_reflect::{Reflect, ReflectError, Value, diff};
use slime_serialize::{Composer, Document, Mode, ReadError, References};

use crate::components::Unsaved;
use crate::entity::Entity;
use crate::identity::PersistentId;
use crate::world::{Identity, World};

/// The count of the entities created outside scenes.
const NEXT_CREATED: &str = "next_created";
/// The scenes spawned, by their keys: their names and contents.
const SCENES: &str = "scenes";
/// The entities and the components removed.
const REMOVED: &str = "removed";
/// The entities given another parent, or none.
const PARENTS: &str = "parents";
/// The entities created outside scenes, whole.
const CREATED: &str = "created";
/// The components added, whole.
const ADDED: &str = "added";
/// What changed of the components, in parts.
const CHANGED: &str = "changed";

/// Why a world cannot be saved, or a save loaded.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DocumentError {
    /// A value that the world holds cannot be written, as `slime-serialize` says: a number out of
    /// its field's range, a reference to an entity without an identity.
    #[error("cannot be saved: {0}")]
    Write(#[source] ReflectError),
    /// The save cannot be read: it is not TOML, is of a later version, or is not a save.
    #[error("the save cannot be read")]
    Read(#[source] ReadError),
    /// The save creates an entity under an identity that is not one of an entity created outside a
    /// scene, `created/<number>`.
    #[error(
        "the save creates `{0}`, which is not the identity of an entity created outside a scene"
    )]
    NotCreated(String),
    /// The save creates an entity under an identity that an entity of the world has: the game made
    /// more entities at the start of its run than when the save was made.
    #[error("the save creates `{0}`, an identity that an entity of the world has already")]
    IdentityInUse(PersistentId),
}

/// What loading a save left aside, and why: what the save holds of entities or components that
/// the world, made from the scenes as they are now, no longer has.
#[derive(Debug, Default)]
pub struct Loaded {
    /// The keys of the scenes that the save was made on whose content changed since, or that the
    /// world no longer spawns.
    pub scenes_changed: Vec<String>,
    /// The differences that could not be applied.
    pub set_aside: Vec<SetAside>,
    /// What the save holds that this program does not know, left out: fields, types.
    pub warnings: Vec<ReadError>,
}

/// A difference of a save that could not be applied, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct SetAside {
    /// The entity it is about, by its persistent identity.
    pub entity: String,
    /// The component it is about, if it is about one.
    pub component: Option<String>,
    /// Why it was set aside.
    pub reason: String,
}

/// Names entities by their persistent identities, in a world's documents.
struct Names<'w>(&'w World);

impl References for Names<'_> {
    /// An entity that the save leaves out has no name in it.
    fn name(&self, entity: u64) -> Option<String> {
        let entity = Entity::from_value(&Value::Entity(entity)).ok()?;
        if self.0.unsaved(entity) {
            return None;
        }
        self.0.persistent_id(entity).map(|id| id.to_string())
    }

    fn entity(&self, name: &str) -> Option<u64> {
        let entity = self.0.find(&name.parse().ok()?)?;
        match entity.to_value() {
            Value::Entity(bits) => Some(bits),
            _ => None,
        }
    }
}

impl World {
    /// What differs between this world and `start`, the world that the game makes at the start of a
    /// run, from the same scenes and as it prepares them, as the text of a save
    /// (`docs/adr/0087-the-worlds-documents.md`): the entities removed; those created while the
    /// world ran, whole; the components added, whole, and removed; the parents changed; and what
    /// changed of the other components, in parts. Entities are matched by their persistent
    /// identities; those that hold [`Unsaved`], or whose ancestor does, are left out, and so are the
    /// fields marked transient. The save also holds the scenes that the world spawned, so that
    /// loading it says which changed since, and the count of the entities created outside scenes.
    ///
    /// The same worlds give the same bytes: entities in the order of their identities, components
    /// in the order of their names.
    ///
    /// # Errors
    ///
    /// [`DocumentError::Write`] if a value cannot be written: a number out of its field's range, a
    /// reference to an entity that has no identity or is left out.
    pub fn save(&self, start: &World) -> Result<String, DocumentError> {
        let names = Names(self);
        let mut save = Composer::new(&self.registry)
            .naming(&names)
            .without_transient();
        let mut put = |place: &[&str], ty: &str, value: Value| {
            save.put(place, ty, &value).map_err(DocumentError::Write)
        };
        put(&[NEXT_CREATED], "u64", Value::UInt(self.next_created))?;
        for (key, scene) in &self.scenes {
            put(
                &[SCENES, key, "name"],
                "String",
                Value::Text(scene.name.clone()),
            )?;
            let content = Value::Text(format!("{:016x}", scene.content));
            put(&[SCENES, key, "content"], "String", content)?;
        }
        let now = self.saved_entities();
        let before = start.saved_entities();
        let removed: Vec<Value> = before
            .keys()
            .filter(|id| !now.contains_key(*id))
            .map(|id| Value::Text(id.to_string()))
            .collect();
        if !removed.is_empty() {
            put(&[REMOVED, "entities"], "Vec<String>", Value::List(removed))?;
        }
        let parent_of = |world: &World, entity| {
            world
                .parent(entity)
                .and_then(|parent| world.persistent_id(parent))
        };
        for (id, &entity) in &now {
            let key = id.to_string();
            let Some(&old) = before.get(id) else {
                continue;
            };
            let gone: Vec<Value> = start
                .component_names_sorted(old)
                .into_iter()
                .filter(|name| self.component_value(entity, name).is_none())
                .map(|name| Value::Text(name.to_owned()))
                .collect();
            if !gone.is_empty() {
                put(
                    &[REMOVED, "components", &key],
                    "Vec<String>",
                    Value::List(gone),
                )?;
            }
            let parent = parent_of(self, entity);
            if parent != parent_of(start, old) {
                put(&[PARENTS, &key], "Option<String>", text_option(parent))?;
            }
        }
        drop(put);
        for (id, &entity) in &now {
            let key = id.to_string();
            match before.get(id) {
                None => {
                    let parent = text_option(parent_of(self, entity));
                    save.put(&[CREATED, &key, "parent"], "Option<String>", &parent)
                        .map_err(DocumentError::Write)?;
                    for name in self.component_names_sorted(entity) {
                        let Some(value) = self.component_value(entity, name) else {
                            continue;
                        };
                        save.put(&[CREATED, &key, "components", name], name, &value)
                            .map_err(DocumentError::Write)?;
                    }
                }
                Some(&old) => {
                    for name in self.component_names_sorted(entity) {
                        let Some(value) = self.component_value(entity, name) else {
                            continue;
                        };
                        match start.component_value(old, name) {
                            None => save.put(&[ADDED, &key, name], name, &value),
                            Some(was) => {
                                let part = self
                                    .registry
                                    .difference(name, &was, &value, false)
                                    .map_err(DocumentError::Write)?;
                                match part {
                                    Some(part) => {
                                        save.put_part(&[CHANGED, &key, name], name, &part)
                                    }
                                    None => Ok(()),
                                }
                            }
                        }
                        .map_err(DocumentError::Write)?;
                    }
                }
            }
        }
        Ok(save.finish())
    }

    /// Applies the save `text` to this world, which the game has made as at the start of a run,
    /// from the scenes as they are now ([`World::save`]): the entities removed are despawned, those
    /// created spawned under their identities, the components added, removed and changed, the
    /// parents set, and the count of the entities created outside scenes kept. A save is read
    /// leniently: what this program does not know is left out, with a warning. A difference whose
    /// entity or component the world no longer has, or that the component's type refuses, is set
    /// aside, and the rest applied; the scenes that changed since the save are named.
    ///
    /// # Errors
    ///
    /// [`DocumentError::Read`] if the save is not one: not TOML, of a later version of the format or
    /// of a type, too large; [`DocumentError::IdentityInUse`] if it creates an entity under an
    /// identity that an entity of the world has. The world is unchanged then.
    pub fn load(&mut self, text: &str) -> Result<Loaded, DocumentError> {
        let mut loaded = Loaded::default();
        let next_created = self.next_created;
        // Created first, so that the values read next can refer to them.
        let created = self.create(text)?;
        let read = match self.read_save(text, &mut loaded) {
            Ok(read) => read,
            Err(error) => {
                for entity in created {
                    self.despawn(entity);
                }
                // Numbers from the count's again, larger than every number held.
                self.created.forget_destroyed(&self.entities);
                self.next_created = next_created;
                return Err(error);
            }
        };
        self.apply(read, &mut loaded);
        Ok(loaded)
    }

    /// The first difference between what this world and `other` would save, said in a sentence, or
    /// `None` if there is none: an entity that one of them holds and the other does not, by its
    /// persistent identity; another parent; other components; a component's value, floats compared
    /// by their bits. The entities that hold [`Unsaved`], or whose ancestor does, are left out, as a
    /// save leaves them out; transient fields are compared. It tells whether a world loaded from a
    /// save is the world saved, or whether two runs went the same way.
    ///
    /// ```
    /// use slime_world::{Name, World};
    ///
    /// let (mut a, mut b) = (World::new(), World::new());
    /// let (door, gate) = (a.spawn(), b.spawn());
    /// a.insert(door, Name("door".to_owned()));
    /// b.insert(gate, Name("door".to_owned()));
    /// assert_eq!(a.first_difference(&b), None);
    /// b.insert(gate, Name("gate".to_owned()));
    /// assert!(a.first_difference(&b).is_some());
    /// ```
    pub fn first_difference(&self, other: &World) -> Option<String> {
        let (ours, theirs) = (self.saved_entities(), other.saved_entities());
        if let Some(id) = ours.keys().find(|id| !theirs.contains_key(id)) {
            return Some(format!("`{id}` is only in the first world"));
        }
        if let Some(id) = theirs.keys().find(|id| !ours.contains_key(id)) {
            return Some(format!("`{id}` is only in the second world"));
        }
        let parent = |world: &World, entity| {
            world
                .parent(entity)
                .and_then(|parent| world.persistent_id(parent))
        };
        for (id, &entity) in &ours {
            let other_entity = theirs[id];
            let parents = (parent(self, entity), parent(other, other_entity));
            if parents.0 != parents.1 {
                return Some(format!(
                    "`{id}` has the parent {:?} in the first world, and {:?} in the second",
                    parents.0.map(|id| id.to_string()),
                    parents.1.map(|id| id.to_string())
                ));
            }
            let names = self.component_names_sorted(entity);
            let other_names = other.component_names_sorted(other_entity);
            if names != other_names {
                return Some(format!(
                    "`{id}` holds {names:?} in the first world, and {other_names:?} in the second"
                ));
            }
            for name in names {
                match (
                    self.component_value(entity, name),
                    other.component_value(other_entity, name),
                ) {
                    (Some(a), Some(b)) if diff(&a, &b).is_empty() => {}
                    (a, b) => {
                        return Some(format!(
                            "`{id}`: its `{name}` is {a:?} in the first world, and {b:?} in the \
                             second"
                        ));
                    }
                }
            }
        }
        None
    }

    /// Spawns the entities that the save `text` creates, under their identities, and returns them.
    fn create(&mut self, text: &str) -> Result<Vec<Entity>, DocumentError> {
        let document =
            Document::parse(&self.registry, text, Mode::Lenient).map_err(DocumentError::Read)?;
        let keys = document.keys(&[CREATED]);
        drop(document);
        let mut numbers = Vec::new();
        for key in keys {
            let number = key
                .parse::<PersistentId>()
                .ok()
                .and_then(|id| id.created_number())
                .ok_or_else(|| DocumentError::NotCreated(key.clone()))?;
            if self.created.find(number, &self.entities).is_some() {
                return Err(DocumentError::IdentityInUse(PersistentId::created(number)));
            }
            numbers.push(number);
        }
        let created: Vec<(u64, Entity)> = numbers
            .into_iter()
            .map(|number| {
                let entity = self.spawn_bare();
                self.nodes[entity.index() as usize].identity = Identity::Created(number);
                // The count stays larger than every number given, whatever the save's says.
                self.next_created = self.next_created.max(number + 1);
                (number, entity)
            })
            .collect();
        self.created.extend(created.iter().copied());
        Ok(created.into_iter().map(|(_, entity)| entity).collect())
    }

    /// Reads all that the save `text` holds but the entities it creates, which are spawned already.
    fn read_save(&self, text: &str, loaded: &mut Loaded) -> Result<Save, DocumentError> {
        let names = Names(self);
        let mut document =
            Document::parse(&self.registry, text, Mode::Lenient).map_err(DocumentError::Read)?;
        let mut save = Save {
            next_created: match document.read(&[NEXT_CREATED], "u64", None) {
                Ok(Some(Value::UInt(next))) => next,
                Ok(_) => 1,
                Err(error) => return Err(DocumentError::Read(error)),
            },
            ..Save::default()
        };
        for key in document.keys(&[SCENES]) {
            let content = document
                .read(&[SCENES, &key, "content"], "String", None)
                .map_err(DocumentError::Read)?;
            let same = match (self.scenes.get(&key), content) {
                (Some(scene), Some(Value::Text(content))) => {
                    format!("{:016x}", scene.content) == content
                }
                _ => false,
            };
            if !same {
                loaded.scenes_changed.push(key);
            }
        }
        let mut aside = |entity: &str, component: Option<&str>, reason: String| {
            loaded.set_aside.push(SetAside {
                entity: entity.to_owned(),
                component: component.map(str::to_owned),
                reason,
            });
        };
        if let Some(Value::List(entities)) = document
            .read(&[REMOVED, "entities"], "Vec<String>", None)
            .map_err(DocumentError::Read)?
        {
            save.removed = texts(entities);
        }
        for key in document.keys(&[REMOVED, "components"]) {
            if let Some(Value::List(names)) = document
                .read(&[REMOVED, "components", &key], "Vec<String>", None)
                .map_err(DocumentError::Read)?
            {
                save.removed_components.push((key, texts(names)));
            }
        }
        for key in document.keys(&[PARENTS]) {
            if let Some(parent) = document
                .read(&[PARENTS, &key], "Option<String>", None)
                .map_err(DocumentError::Read)?
            {
                save.parents.push((key, option_text(&parent)));
            }
        }
        for key in document.keys(&[CREATED]) {
            if let Some(parent) = document
                .read(&[CREATED, &key, "parent"], "Option<String>", None)
                .map_err(DocumentError::Read)?
            {
                save.parents.push((key.clone(), option_text(&parent)));
            }
            for name in document.keys(&[CREATED, &key, "components"]) {
                match document.read(&[CREATED, &key, "components", &name], &name, Some(&names)) {
                    Ok(Some(value)) => save.added.push((key.clone(), name, value)),
                    Ok(None) => {}
                    Err(error) => aside(&key, Some(&name), error.to_string()),
                }
            }
        }
        for key in document.keys(&[ADDED]) {
            for name in document.keys(&[ADDED, &key]) {
                match document.read(&[ADDED, &key, &name], &name, Some(&names)) {
                    Ok(Some(value)) => save.added.push((key.clone(), name, value)),
                    Ok(None) => {}
                    Err(error) => aside(&key, Some(&name), error.to_string()),
                }
            }
        }
        for key in document.keys(&[CHANGED]) {
            for name in document.keys(&[CHANGED, &key]) {
                match document.read_part(&[CHANGED, &key, &name], &name, Some(&names)) {
                    Ok(Some(part)) => save.changed.push((key.clone(), name, part)),
                    Ok(None) => {}
                    Err(error) => aside(&key, Some(&name), error.to_string()),
                }
            }
        }
        loaded.warnings.extend(document.warnings());
        Ok(save)
    }

    /// Applies what a save holds, setting aside what cannot be.
    fn apply(&mut self, save: Save, loaded: &mut Loaded) {
        let mut aside = |entity: &str, component: Option<&str>, reason: &str| {
            loaded.set_aside.push(SetAside {
                entity: entity.to_owned(),
                component: component.map(str::to_owned),
                reason: reason.to_owned(),
            });
        };
        const GONE: &str = "the world has no entity of this identity";
        for key in &save.removed {
            match self.entity_named(key) {
                Some(entity) => {
                    self.despawn(entity);
                }
                None => aside(key, None, GONE),
            }
        }
        for (key, parent) in &save.parents {
            let Some(entity) = self.entity_named(key) else {
                aside(key, None, GONE);
                continue;
            };
            let parent = match parent {
                Some(parent) => match self.entity_named(parent) {
                    Some(parent) => Some(parent),
                    None => {
                        aside(key, None, &format!("the world has no parent `{parent}`"));
                        continue;
                    }
                },
                None => None,
            };
            if let Err(error) = self.set_parent(entity, parent) {
                aside(key, None, &error.to_string());
            }
        }
        for (key, names) in &save.removed_components {
            for name in names {
                let removed = self
                    .entity_named(key)
                    .and_then(|entity| self.remove_component_value(entity, name));
                if removed.is_none() {
                    aside(key, Some(name), "the entity holds no such component");
                }
            }
        }
        for (key, name, value) in save.added {
            let Some(entity) = self.entity_named(&key) else {
                aside(&key, Some(&name), GONE);
                continue;
            };
            if let Err(error) = self.insert_component_value(entity, &name, value) {
                aside(&key, Some(&name), &error.to_string());
            }
        }
        for (key, name, part) in save.changed {
            let Some(entity) = self.entity_named(&key) else {
                aside(&key, Some(&name), GONE);
                continue;
            };
            let Some(mut value) = self.component_value(entity, &name) else {
                aside(&key, Some(&name), "the entity holds no such component");
                continue;
            };
            let changed = self
                .registry
                .overlay(&name, &mut value, &part)
                .map_err(|error| error.to_string())
                .and_then(|()| {
                    self.set_component_value(entity, &name, value)
                        .map_err(|error| error.to_string())
                });
            if let Err(reason) = changed {
                aside(&key, Some(&name), &reason);
            }
        }
        self.next_created = self.next_created.max(save.next_created);
    }

    /// The entity of the persistent identity that `text` writes, if one is alive.
    fn entity_named(&self, text: &str) -> Option<Entity> {
        self.find(&text.parse().ok()?)
    }

    /// The entities that a save holds, by their persistent identities: all but those that hold
    /// [`Unsaved`], or whose ancestor does.
    fn saved_entities(&self) -> BTreeMap<PersistentId, Entity> {
        self.entities()
            .filter(|&entity| !self.unsaved(entity))
            .filter_map(|entity| Some((self.persistent_id(entity)?, entity)))
            .collect()
    }

    /// Whether the entity, or one of its ancestors, holds [`Unsaved`].
    fn unsaved(&self, entity: Entity) -> bool {
        let mut next = Some(entity);
        while let Some(entity) = next {
            if self.get::<Unsaved>(entity).is_some() {
                return true;
            }
            next = self.parent(entity);
        }
        false
    }

    /// The names of the types of the components that `entity` holds, in their order.
    fn component_names_sorted(&self, entity: Entity) -> Vec<&str> {
        let mut names = self.component_names(entity);
        names.sort_unstable();
        names
    }
}

/// What a save holds, read, but the entities it creates.
#[derive(Default)]
struct Save {
    next_created: u64,
    removed: Vec<String>,
    removed_components: Vec<(String, Vec<String>)>,
    /// Each entity given another parent, the created ones included, and its parent, if it has one.
    parents: Vec<(String, Option<String>)>,
    /// The components added, of the entities the world had and of those it created: the entity,
    /// the component and its value.
    added: Vec<(String, String, Value)>,
    /// What changed of the components: the entity, the component and the part.
    changed: Vec<(String, String, Value)>,
}

/// The persistent identity `id`, if there is one, as an option of text.
fn text_option(id: Option<PersistentId>) -> Value {
    Value::Option(id.map(|id| Box::new(Value::Text(id.to_string()))))
}

/// The text that `value`, an option of text, holds, if it holds one.
fn option_text(value: &Value) -> Option<String> {
    match value {
        Value::Option(Some(text)) => match &**text {
            Value::Text(text) => Some(text.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// The texts of a list of text.
fn texts(values: Vec<Value>) -> Vec<String> {
    values
        .into_iter()
        .filter_map(|value| match value {
            Value::Text(text) => Some(text),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use glam::{DVec3, Mat4, Vec3};
    use slime_assets::{AssetId, CookedScene, SceneNode};

    use super::*;
    use crate::{Hidden, Meshes, Name, Transform};

    fn node(name: &str, parent: Option<u32>, at: [f32; 3]) -> SceneNode {
        SceneNode {
            name: name.to_owned(),
            parent,
            transform: Mat4::from_translation(Vec3::from(at)).to_cols_array(),
            meshes: vec![AssetId::from_u128(7)],
            light: None,
            extras: Vec::new(),
        }
    }

    fn scene() -> CookedScene {
        CookedScene {
            name: "yard".to_owned(),
            nodes: vec![
                node("gate", None, [0.0, 0.0, 0.0]),
                node("post", Some(0), [1.0, 0.0, 0.0]),
                node("well", None, [5.0, 0.0, 2.0]),
                node("tree", None, [-3.0, 0.0, 4.0]),
            ],
        }
    }

    /// A world as a game makes it at the start of a run.
    fn start() -> World {
        let mut world = World::new();
        world.spawn_scene("yard", &scene()).expect("spawned");
        world
    }

    fn entity(world: &World, id: &str) -> Entity {
        world
            .find(&id.parse().expect("an identity"))
            .expect("alive")
    }

    /// A run: a value changed, a component added and one removed, an entity removed, one moved to
    /// another parent, two created, one of them for a moment.
    fn played() -> World {
        let mut world = start();
        let well = entity(&world, "yard/well");
        world.insert(well, Transform::from_translation(DVec3::new(6.5, 0.0, 2.0)));
        world.insert(entity(&world, "yard/gate"), Hidden);
        world.remove::<Meshes>(entity(&world, "yard/post"));
        world.despawn(entity(&world, "yard/tree"));
        let post = entity(&world, "yard/post");
        world.set_parent(post, Some(well)).expect("a parent");
        let torch = world.spawn();
        world.insert(torch, Name("torch".to_owned()));
        world.insert(
            torch,
            Transform::from_translation(DVec3::new(0.5, 1.0, 0.0)),
        );
        world.set_parent(torch, Some(well)).expect("a parent");
        let spark = world.spawn();
        world.insert(spark, Unsaved);
        world
    }

    #[test]
    fn a_world_saved_and_loaded_into_its_start_is_the_world_saved() {
        let played = played();
        let save = played.save(&start()).expect("saved");
        let mut loaded = start();
        let report = loaded.load(&save).expect("loaded");
        assert!(report.set_aside.is_empty(), "{:?}", report.set_aside);
        assert!(report.scenes_changed.is_empty() && report.warnings.is_empty());
        assert_eq!(played.first_difference(&loaded), None, "{save}");
        assert_eq!(
            loaded.save(&start()).expect("saved again"),
            save,
            "the same bytes"
        );
        // The entity created for a moment is left out, and its number not given again.
        assert!(
            loaded
                .find(&"created/2".parse().expect("an identity"))
                .is_none()
        );
        assert_eq!(loaded.spawn(), entity(&loaded, "created/3"));
        assert_eq!(
            save,
            r#"next_created = 3

[scenes]
yard = { name = "yard", content = "SCENE" }

[removed]
entities = ["yard/tree"]

[removed.components]
"yard/post" = ["Meshes"]

[parents]
"yard/post" = ["yard/well"]

[created."created/1"]
parent = ["yard/well"]

[created."created/1".components]
Name = "torch"

[created."created/1".components.Transform]
translation = { x = 0.5, y = 1.0, z = 0.0 }
rotation = { x = 0.0, y = 0.0, z = 0.0, w = 1.0 }
scale = 1.0
stretch = { x = 1.0, y = 1.0, z = 1.0 }

[added."yard/gate"]
Hidden = {}

[changed."yard/well".Transform]
translation = { x = 6.5 }
"#
            .replace("SCENE", &format!("{:016x}", played.scenes["yard"].content))
        );
    }

    /// The scene of [`scene`] as a new export makes it: the well moved, the tree gone, the gate
    /// renamed and a bench added.
    fn scene_changed() -> CookedScene {
        CookedScene {
            name: "yard".to_owned(),
            nodes: vec![
                node("door", None, [0.0, 0.0, 0.0]),
                node("post", Some(0), [1.0, 0.0, 0.0]),
                node("well", None, [9.0, 0.0, 2.0]),
                node("bench", None, [2.0, 0.0, -2.0]),
            ],
        }
    }

    #[test]
    fn a_save_loaded_on_a_scene_changed_since_applies_what_it_can_and_says_what_it_cannot() {
        let save = played().save(&start()).expect("saved");
        let mut world = World::new();
        world
            .spawn_scene("yard", &scene_changed())
            .expect("spawned");
        let report = world.load(&save).expect("loaded");
        assert_eq!(report.scenes_changed, ["yard"]);
        let reported: Vec<(&str, Option<&str>)> = report
            .set_aside
            .iter()
            .map(|aside| (aside.entity.as_str(), aside.component.as_deref()))
            .collect();
        assert_eq!(
            reported,
            [("yard/tree", None), ("yard/gate", Some("Hidden"))],
            "the tree is gone already, the gate renamed"
        );
        // The well keeps what the save changed of it, the post was taken out and moved under it,
        // the torch was created under it, and the bench, new, is there.
        let well = entity(&world, "yard/well");
        assert_eq!(
            world.get::<Transform>(well).map(|at| at.translation),
            Some(DVec3::new(6.5, 0.0, 2.0))
        );
        let post = entity(&world, "yard/post");
        assert_eq!(world.parent(post), Some(well));
        assert!(world.get::<Meshes>(post).is_none());
        assert_eq!(world.parent(entity(&world, "created/1")), Some(well));
        entity(&world, "yard/bench");
    }

    #[test]
    fn a_save_that_is_not_one_is_refused_and_the_world_left_as_it_was() {
        let save = played().save(&start()).expect("saved");
        for (text, problem) in [
            ("next_created = [", "not TOML"),
            ("\"$format\" = 2\n", "newer"),
            ("\"$versions\" = { Transform = 9 }\n", "newer"),
            ("[created.\"yard/gate\"]\nparent = []\n", "not the identity"),
            // Its entity created, then taken back when the count cannot be read.
            (
                "next_created = true\n[created.\"created/7\"]\n",
                "cannot be read",
            ),
        ] {
            let mut world = start();
            let error = world.load(text).expect_err(text);
            assert!(
                format!(
                    "{error}: {}",
                    std::error::Error::source(&error).map_or(String::new(), ToString::to_string)
                )
                .contains(problem),
                "{text}: {error:?}"
            );
            assert_eq!(
                world.first_difference(&start()),
                None,
                "{text}: the world as it was"
            );
            let next = world.spawn();
            assert_eq!(
                world.persistent_id(next),
                Some(PersistentId::created(1)),
                "{text}: the count as it was"
            );
        }
        // A save whose count is lower than a number it gives: the world's stays larger.
        let mut world = start();
        world
            .load("next_created = 2\n[created.\"created/5\"]\n")
            .expect("loaded");
        let next = world.spawn();
        assert_eq!(world.persistent_id(next), Some(PersistentId::created(6)));
        // A second load creates its entities under identities that the first gave already.
        let mut world = start();
        world.load(&save).expect("loaded");
        let error = world.load(&save).expect_err("twice");
        assert!(matches!(error, DocumentError::IdentityInUse(id) if id.to_string() == "created/1"));
        // Truncated anywhere, a save gives an error or loads what it holds, and never panics.
        for end in (0..save.len()).filter(|&end| save.is_char_boundary(end)) {
            let _ = start().load(&save[..end]);
        }
    }

    /// A component that refers to another entity, and holds what saves leave out.
    #[derive(Clone, Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Rope")]
    struct Rope {
        to: Entity,
        #[reflect(transient, default)]
        swing: f32,
    }
    impl crate::Component for Rope {}

    #[test]
    fn references_to_entities_are_saved_as_identities_and_transient_fields_left_out() {
        let start = || {
            let mut world = super::tests::start();
            world.register::<Rope>();
            world
        };
        let mut played = start();
        let (gate, well) = (entity(&played, "yard/gate"), entity(&played, "yard/well"));
        played.insert(
            gate,
            Rope {
                to: well,
                swing: 0.5,
            },
        );
        let save = played.save(&start()).expect("saved");
        assert!(
            save.contains("[added.\"yard/gate\"]\n\"test::Rope\" = { to = \"yard/well\" }\n"),
            "{save}"
        );
        assert!(!save.contains("swing"), "{save}");
        let mut loaded = start();
        loaded.load(&save).expect("loaded");
        let rope = loaded
            .get::<Rope>(entity(&loaded, "yard/gate"))
            .expect("a rope");
        assert_eq!(rope.to, entity(&loaded, "yard/well"));
        assert_eq!(rope.swing, 0.0, "its default");
        // An entity left out of the save cannot be referred to.
        let spark = played.spawn();
        played.insert(spark, Unsaved);
        played.insert(
            gate,
            Rope {
                to: spark,
                swing: 0.0,
            },
        );
        let error = played
            .save(&start())
            .expect_err("a reference to an unsaved entity");
        assert!(error.to_string().contains("cannot be saved"), "{error}");
    }
}
