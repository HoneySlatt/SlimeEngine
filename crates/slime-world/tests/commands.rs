//! Commands done, undone and done again, and written as text: each leaves the scene file that the
//! world writes as it was, identities, ranks and references included
//! (`docs/adr/0109-stable-references-and-undo.md`).

use slime_assets::AssetId;
use slime_reflect::{Reflect, Value};
use slime_world::{
    Command, CommandTextError, Component, Entity, Hidden, Name, PersistentId, SceneLibrary,
    Transform, World, WorldError,
};

/// What an entity looks at.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Aim")]
struct Aim {
    at: Entity,
}
impl Component for Aim {}

/// How a lamp glows.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Glow")]
struct Glow {
    #[reflect(range(min = 0.0))]
    intensity: f32,
}
impl Component for Glow {}

const YARD: AssetId = AssetId::from_u128(1);
const LAMP: AssetId = AssetId::from_u128(2);

/// A lamp on a post, its light aiming at the post.
const LAMP_FILE: &str = r#"
[entities.post.components]
Name = "post"

[entities.light]
parent = "post"

[entities.light.components]
"test::Glow" = { intensity = 40.0 }
"test::Aim" = { at = "post" }
"#;

/// Two lamps, the gate's moved and dimmer, the porch's light removed; a well and its bucket; a sign
/// that aims at the bucket.
const YARD_FILE: &str = r#"
[assets]
lamp = { id = "00000000-0000-0000-0000-000000000002", path = "lamp.scene" }

[entities.gate]
prefab = "lamp"

[entities.porch]
prefab = "lamp"

[entities.well.components]
Name = "well"
Transform = { translation = { x = 2.0, y = 0.0, z = 0.0 } }

[entities.bucket]
parent = "well"

[entities.bucket.components]
Name = "bucket"

[entities.rope]
parent = "well"

[entities.rope.components]
Name = "rope"

[entities.sign.components]
Name = "sign"
"test::Aim" = { at = "bucket" }

[changed.gate.Transform]
translation = { x = 4.0 }

[changed."gate/light"."test::Glow"]
intensity = 10.0

[removed]
entities = ["porch/light"]
"#;

fn library() -> SceneLibrary {
    let mut scenes = SceneLibrary::new();
    scenes.insert_file(LAMP, "lamp", LAMP_FILE);
    scenes.insert_file(YARD, "yard", YARD_FILE);
    scenes
}

/// The yard spawned under `yard`, and the library it was resolved from.
fn yard() -> (World, SceneLibrary, Entity) {
    let mut world = World::new();
    world.register::<Aim>();
    world.register::<Glow>();
    let mut scenes = library();
    let scene = scenes.resolve(world.registry(), YARD).expect("the yard");
    let root = world.spawn_scene("yard", &scene).expect("spawned");
    (world, scenes, root)
}

/// The scene file that the world writes of the yard.
fn file(world: &mut World, root: Entity) -> String {
    world.write_scene(root).expect("the yard is written").text
}

fn id(text: &str) -> PersistentId {
    text.parse().expect("an identity")
}

/// The commands of each kind, but a restoration, on the yard.
fn commands(world: &World, scenes: &mut SceneLibrary) -> Vec<Command> {
    let lamp = scenes.resolve(world.registry(), LAMP).expect("the lamp");
    let sign = world.find(&id("yard/sign")).expect("the sign");
    vec![
        Command::SetProperty {
            entity: id("yard/gate"),
            component: "Transform".to_owned(),
            path: "translation.x".parse().expect("a path"),
            value: Value::F64(6.0),
        },
        Command::Create {
            entity: id("yard/torch-0rdx"),
            parent: Some(id("yard/well")),
            rank: Some(0),
            components: vec![
                ("Name".to_owned(), Name("torch".to_owned()).to_value()),
                ("test::Aim".to_owned(), Aim { at: sign }.to_value()),
            ],
        },
        Command::Place {
            entity: id("yard/lamp-k3f9"),
            parent: Some(id("yard")),
            rank: Some(1),
            prefab: lamp,
        },
        Command::Delete {
            entity: id("yard/well"),
        },
        Command::Delete {
            entity: id("yard/gate/light"),
        },
        Command::Delete {
            entity: id("yard/gate"),
        },
        Command::Reparent {
            entity: id("yard/rope"),
            parent: Some(id("yard")),
            rank: Some(0),
        },
        Command::Reparent {
            entity: id("yard/rope"),
            parent: Some(id("yard/well")),
            rank: Some(0),
        },
        Command::Reparent {
            entity: id("yard/sign"),
            parent: Some(id("yard/well")),
            rank: None,
        },
        Command::Insert {
            entity: id("yard/sign"),
            component: "Hidden".to_owned(),
            value: Hidden.to_value(),
        },
        Command::Insert {
            entity: id("yard/sign"),
            component: "Name".to_owned(),
            value: Name("old sign".to_owned()).to_value(),
        },
        Command::Remove {
            entity: id("yard/sign"),
            component: "test::Aim".to_owned(),
        },
    ]
}

#[test]
fn each_command_done_undone_and_redone_leaves_the_file_as_it_was() {
    let (world, mut scenes, _) = yard();
    let count = commands(&world, &mut scenes).len();
    for index in 0..count {
        let (mut world, mut scenes, root) = yard();
        let command = commands(&world, &mut scenes).remove(index);
        let before = file(&mut world, root);
        let undo = world.apply(&command).expect("applied");
        let after = file(&mut world, root);
        assert_ne!(after, before, "{command:?} changes the file");
        let redo = world.apply(&undo).expect("undone");
        assert_eq!(file(&mut world, root), before, "{command:?} undone");
        let undo = world.apply(&redo).expect("redone");
        assert_eq!(file(&mut world, root), after, "{command:?} redone");
        world.apply(&undo).expect("undone again");
        assert_eq!(file(&mut world, root), before, "{command:?} undone again");
    }
}

#[test]
fn a_deleted_subtree_comes_back_named_by_what_named_it() {
    let (mut world, _, root) = yard();
    let sign = world.find(&id("yard/sign")).expect("the sign");
    let bucket = world.find(&id("yard/bucket")).expect("the bucket");
    assert_eq!(world.get::<Aim>(sign), Some(&Aim { at: bucket }));
    let ranks = |world: &World| {
        ["yard/bucket", "yard/rope"].map(|name| world.rank(world.find(&id(name)).expect("alive")))
    };
    let before = (ranks(&world), file(&mut world, root));
    let restore = world
        .apply(&Command::Delete {
            entity: id("yard/well"),
        })
        .expect("deleted");
    let Command::Restore(deleted) = &restore else {
        panic!("a deletion is undone by a restoration, not {restore:?}");
    };
    assert_eq!(deleted.len(), 3, "the well, its bucket and its rope");
    assert!(world.find(&id("yard/bucket")).is_none());
    world.apply(&restore).expect("restored");
    let bucket = world.find(&id("yard/bucket")).expect("the bucket is back");
    assert_eq!(world.get::<Aim>(sign), Some(&Aim { at: bucket }));
    assert_eq!((ranks(&world), file(&mut world, root)), before);
}

#[test]
fn a_restoration_is_refused_while_an_identity_is_taken_and_changes_nothing() {
    let (mut world, _, root) = yard();
    let restore = world
        .apply(&Command::Delete {
            entity: id("yard/well"),
        })
        .expect("deleted");
    world
        .apply(&Command::Create {
            entity: id("yard/rope"),
            parent: None,
            rank: None,
            components: Vec::new(),
        })
        .expect("the rope's identity is free");
    let before = file(&mut world, root);
    let count = world.len();
    assert_eq!(
        world.apply(&restore),
        Err(WorldError::IdentityInUse(id("yard/rope")))
    );
    assert_eq!((world.len(), file(&mut world, root)), (count, before));
}

#[test]
fn a_command_refused_changes_nothing() {
    let (mut world, mut scenes, root) = yard();
    let lamp = scenes.resolve(world.registry(), LAMP).expect("the lamp");
    let before = file(&mut world, root);
    let count = world.len();
    let refused = [
        (
            Command::Create {
                entity: id("yard/well"),
                parent: None,
                rank: None,
                components: Vec::new(),
            },
            WorldError::IdentityInUse(id("yard/well")),
        ),
        (
            Command::Create {
                entity: id("cellar/well"),
                parent: None,
                rank: None,
                components: Vec::new(),
            },
            WorldError::NotInScene(id("cellar/well")),
        ),
        (
            Command::Place {
                entity: id("yard/gate/light/again"),
                parent: None,
                rank: None,
                prefab: lamp.clone(),
            },
            WorldError::NotInScene(id("yard/gate/light/again")),
        ),
        (
            Command::Place {
                entity: id("yard/lamp"),
                parent: Some(id("yard/nowhere")),
                rank: None,
                prefab: lamp,
            },
            WorldError::NotFound(id("yard/nowhere")),
        ),
        (
            Command::Reparent {
                entity: id("yard/well"),
                parent: Some(id("yard/bucket")),
                rank: None,
            },
            WorldError::Cycle {
                child: world.find(&id("yard/well")).expect("the well"),
                parent: world.find(&id("yard/bucket")).expect("the bucket"),
            },
        ),
        (
            Command::Remove {
                entity: id("yard/well"),
                component: "test::Aim".to_owned(),
            },
            WorldError::NoComponent {
                entity: world.find(&id("yard/well")).expect("the well"),
                component: "test::Aim".to_owned(),
            },
        ),
    ];
    for (command, error) in refused {
        assert_eq!(world.apply(&command), Err(error), "{command:?}");
    }
    let torch = Command::Create {
        entity: id("yard/torch-0rdx"),
        parent: None,
        rank: None,
        components: vec![
            ("Name".to_owned(), Name("torch".to_owned()).to_value()),
            ("test::Glow".to_owned(), Glow { intensity: -1.0 }.to_value()),
        ],
    };
    let refused = world.apply(&torch);
    assert!(
        matches!(
            &refused,
            Err(WorldError::Refused { component, .. }) if component == "test::Glow"
        ),
        "{refused:?}"
    );
    assert_eq!((world.len(), file(&mut world, root)), (count, before));
}

#[test]
fn commands_written_as_text_are_read_back_the_same() {
    let (world, mut scenes, _) = yard();
    for command in commands(&world, &mut scenes) {
        let text = command.write(&world).expect("written");
        let read = Command::read(&text, &world, &mut scenes).expect("read back");
        assert_eq!(read, command, "{text}");
    }
    let create = commands(&world, &mut scenes).remove(1);
    assert_eq!(
        create.write(&world).expect("written"),
        "[Create]\nentity = \"yard/torch-0rdx\"\nparent = \"yard/well\"\nrank = 0\n\n\
         [Create.components]\nName = \"torch\"\n\"test::Aim\" = { at = \"yard/sign\" }\n"
    );
}

#[test]
fn text_that_is_not_a_command_is_refused_with_the_reason() {
    let (world, mut scenes, _) = yard();
    let read = |text: &str| Command::read(text, &world, &mut library()).unwrap_err();
    assert_eq!(
        read("[Destroy]\nentity = \"yard/well\"\n").to_string(),
        "`Destroy` is not a kind of command: `SetProperty`, `Create`, `Place`, `Delete`, \
         `Reparent`, `Insert`, `Remove`"
    );
    assert_eq!(
        read("[Delete]\nentity = \"yard/well\"\nrank = 2\n").to_string(),
        "`Delete` has no key `rank`: its keys are entity"
    );
    assert_eq!(read("[Delete]\n").to_string(), "`Delete` has no `entity`");
    assert_eq!(
        read("[Create]\nentity = \"yard/x\"\ncomponents = 5\n").to_string(),
        "`Create`'s `components` is a table of components by their types' names"
    );
    assert!(matches!(
        read("[SetProperty]\nentity = \"yard/nowhere\"\ncomponent = \"Name\"\nvalue = \"x\"\n"),
        CommandTextError::World(WorldError::NotFound(_))
    ));
    assert!(matches!(
        read(
            "[Insert]\nentity = \"yard/well\"\ncomponent = \"test::Aim\"\nvalue = { at = \"yard/nowhere\" }\n"
        ),
        CommandTextError::Read(_)
    ));
    let none = Command::read(
        "[Insert]\nentity = \"yard/well\"\ncomponent = \"test::Aim\"\nvalue = { at = \"\" }\n",
        &world,
        &mut scenes,
    )
    .expect("`\"\"` names no entity");
    assert!(matches!(none, Command::Insert { value, .. } if Aim::from_value(&value).is_ok()));
    let mut world = world;
    let well = world.find(&id("yard/well")).expect("the well");
    let deleted = world
        .apply(&Command::Delete {
            entity: id("yard/well"),
        })
        .expect("deleted");
    assert!(matches!(
        deleted.write(&world),
        Err(CommandTextError::Restore)
    ));
    assert!(!world.contains(well));
}

#[test]
fn what_the_world_names_is_free_and_written_as_the_file_names_it() {
    let (mut world, _, root) = yard();
    let torch = world.new_id("yard", "torch/lit");
    let [key, own] = torch.parts() else {
        panic!("two parts: {torch}");
    };
    assert_eq!(key, "yard");
    assert!(
        own.starts_with("torch-lit-") && own.len() == "torch-lit-".len() + 4,
        "{own}"
    );
    let entity = world.spawn_as(&torch).expect("free");
    world.insert(entity, Name("torch".to_owned()));
    world.insert(entity, Transform::default());
    world.set_parent(entity, Some(root)).expect("alive");
    let text = file(&mut world, root);
    assert!(
        text.contains(&format!("[entities.{own}.components]")),
        "{text}"
    );
    assert_eq!(world.persistent_id(entity), Some(torch));
}

#[test]
fn a_prefab_placed_is_written_as_a_file_places_it() {
    let (mut world, mut scenes, root) = yard();
    let lamp = scenes.resolve(world.registry(), LAMP).expect("the lamp");
    world
        .apply(&Command::Place {
            entity: id("yard/lamp-k3f9"),
            parent: Some(id("yard")),
            rank: None,
            prefab: lamp,
        })
        .expect("placed");
    let placed = world.find(&id("yard/lamp-k3f9")).expect("the placement");
    assert_eq!(
        world.get::<Name>(placed),
        Some(&Name("lamp-k3f9".to_owned()))
    );
    let text = file(&mut world, root);
    assert!(
        text.contains("[entities.lamp-k3f9]\nprefab = \"lamp\"\n\n"),
        "{text}"
    );
    assert!(
        !text.contains("[removed.components]"),
        "no override of the prefab: {text}"
    );
}
