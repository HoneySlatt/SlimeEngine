//! Commands done, undone and done again, and written as text: each leaves the scene file that the
//! world writes as it was, identities, ranks and references included
//! (`docs/adr/0109-stable-references-and-undo.md`); and the commands that create again what the
//! world holds, read under new identities (`docs/adr/0162-creating-copying-and-arranging.md`).

use slime_assets::AssetId;
use slime_reflect::{Reflect, Value};
use slime_world::{
    Command, CommandTextError, Component, Entity, Hidden, Name, PersistentId, Renaming,
    SceneLibrary, Transform, World, WorldError,
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

/// The texts of the commands that create again `roots` of `world`.
fn copied(world: &World, roots: &[Entity]) -> Vec<String> {
    world
        .creating_commands(roots)
        .expect("copied")
        .iter()
        .map(|command| command.write(world).expect("written"))
        .collect()
}

/// `texts`, commands, read under new identities of the level spawned under `key` in `target` and
/// applied there; the renaming that they were read through.
fn pasted(texts: &[String], target: &mut World, key: &str, scenes: &mut SceneLibrary) -> Renaming {
    let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
    let renaming = Renaming::of_commands(target, key, &texts).expect("renamed");
    for text in texts {
        let command = Command::read_renamed(text, target, scenes, &renaming)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
        target.apply(&command).expect("applied");
    }
    renaming
}

/// Copies of two placements, a well and a sign, under new identities: the placements keep their
/// overrides, the one that lost its light loses it too, the well's children come with it, and the
/// sign's reference to the bucket, copied with it, points at the copy, the original's at the
/// original; the file written holds the copies as it holds what they copy.
#[test]
fn what_is_copied_is_created_again_under_new_identities_its_overrides_and_references_kept() {
    let (mut world, mut scenes, root) = yard();
    let find = |world: &World, text: &str| world.find(&id(text)).expect(text);
    // The gate's light loses its glow, and a flag is put up on the gate's post, beside its prefab.
    for command in [
        Command::Remove {
            entity: id("yard/gate/light"),
            component: "test::Glow".to_owned(),
        },
        Command::Create {
            entity: id("yard/flag-k3f9"),
            parent: Some(id("yard/gate/post")),
            rank: None,
            components: vec![("Name".to_owned(), Name("flag".to_owned()).to_value())],
        },
    ] {
        world.apply(&command).expect("applied");
    }
    let roots =
        ["yard/gate", "yard/porch", "yard/well", "yard/sign"].map(|text| find(&world, text));
    let before = file(&mut world, root);
    // A root given twice is copied once.
    assert_eq!(
        copied(&world, &[roots[2], roots[2]]),
        copied(&world, &[roots[2]])
    );
    let texts = copied(&world, &roots);
    let renaming = pasted(&texts, &mut world, "yard", &mut scenes);
    let copy = |text: &str| renaming.renamed(&id(text));
    let gate = copy("yard/gate");
    assert!(gate.to_string().starts_with("yard/gate-"), "{gate}");
    assert_eq!(copy("yard/gate/light").parts()[..2], gate.parts()[..]);
    let new: std::collections::HashSet<_> = ["yard/gate", "yard/porch", "yard/well", "yard/sign"]
        .map(copy)
        .into_iter()
        .collect();
    assert_eq!(new.len(), 4, "one new identity each");
    // The flag, created on the post, created again on the copy's post, under an identity of its own.
    let flag = world
        .find(&copy("yard/flag-k3f9"))
        .expect("the flag's copy");
    assert!(copy("yard/flag-k3f9").to_string().starts_with("yard/flag-"));
    assert_eq!(world.parent(flag), world.find(&copy("yard/gate/post")));
    let at = |world: &World, text: &PersistentId| world.find(text).expect("copied");
    assert_eq!(
        world
            .get::<Transform>(at(&world, &gate))
            .expect("placed")
            .translation
            .x,
        4.0
    );
    assert_eq!(
        world.get::<Glow>(at(&world, &copy("yard/gate/light"))),
        None,
        "removed, as the gate's"
    );
    assert_eq!(
        world.find(&copy("yard/porch/light")),
        None,
        "lost, as the porch's"
    );
    let well = at(&world, &copy("yard/well"));
    let children: Vec<String> = world
        .children(well)
        .map(|child| world.get::<Name>(child).expect("named").0.clone())
        .collect();
    assert_eq!(children, ["bucket", "rope"]);
    let bucket = at(&world, &copy("yard/bucket"));
    assert_eq!(world.parent(bucket), Some(well));
    assert_eq!(
        world.get::<Aim>(at(&world, &copy("yard/sign"))),
        Some(&Aim { at: bucket })
    );
    assert_eq!(
        world.get::<Aim>(find(&world, "yard/sign")),
        Some(&Aim {
            at: find(&world, "yard/bucket")
        })
    );
    // The file holds what it held, and the copies as it holds what they copy.
    let after = file(&mut world, root);
    let new_id = |text: &str| copy(text).parts()[1].clone();
    for (old, new) in [
        ("gate", new_id("yard/gate")),
        ("porch", new_id("yard/porch")),
    ] {
        assert!(
            after.contains(&format!("[entities.{new}]\nprefab = \"lamp\"")),
            "{old}: {after}"
        );
    }
    assert!(
        after.contains(&format!("[changed.{}.Transform]", new_id("yard/gate"))),
        "{after}"
    );
    assert!(
        after.contains(&format!("\"{}/light\"", new_id("yard/porch"))),
        "removed as the porch's light: {after}"
    );
    // The list of what is removed holds the porch's light and its copy's.
    assert!(after.contains("\"porch/light\""), "{after}");
    for line in before
        .lines()
        .filter(|line| !line.starts_with("entities = ["))
    {
        assert!(after.contains(line), "{line} kept");
    }
}

/// A sign copied alone keeps aiming at the bucket, which was not copied; pasted into another level,
/// whose parent is not there, it goes under that level's root.
#[test]
fn a_reference_outside_the_copy_stays_and_a_paste_elsewhere_goes_under_the_level_s_root() {
    let (mut world, mut scenes, _) = yard();
    let sign = world.find(&id("yard/sign")).expect("the sign");
    let bucket = world.find(&id("yard/bucket")).expect("the bucket");
    let texts = copied(&world, &[sign]);
    let renaming = pasted(&texts, &mut world, "yard", &mut scenes);
    let copy = world
        .find(&renaming.renamed(&id("yard/sign")))
        .expect("copied");
    assert_eq!(world.get::<Aim>(copy), Some(&Aim { at: bucket }));
    // Into a garden, where the well, its bucket and rope come under the garden's root.
    let mut garden = World::new();
    garden.register::<Aim>();
    garden.register::<Glow>();
    scenes.insert_file(AssetId::from_u128(3), "garden", "");
    let scene = scenes
        .resolve(garden.registry(), AssetId::from_u128(3))
        .expect("the garden");
    let root = garden.spawn_scene("garden", &scene).expect("spawned");
    let well = world.find(&id("yard/well")).expect("the well");
    let texts = copied(&world, &[well]);
    let renaming = pasted(&texts, &mut garden, "garden", &mut scenes);
    let copy = garden
        .find(&renaming.renamed(&id("yard/well")))
        .expect("pasted");
    assert!(
        renaming
            .renamed(&id("yard/well"))
            .to_string()
            .starts_with("garden/well-")
    );
    assert_eq!(garden.parent(copy), Some(root));
    assert_eq!(garden.children(copy).count(), 2);
}

/// A house that places a lamp, its light dimmed by the house, placed in a garden that dims it more:
/// copied, the house's copy keeps the garden's override of the light within it, and the file
/// writes it for the copy as it writes it for the house.
#[test]
fn a_copy_keeps_the_overrides_of_a_placement_within_a_placement() {
    let mut scenes = library();
    let house = AssetId::from_u128(4);
    scenes.insert_file(
        house,
        "house",
        r#"
[assets]
lamp = { id = "00000000-0000-0000-0000-000000000002", path = "lamp.scene" }

[entities.porch]
prefab = "lamp"

[changed."porch/light"."test::Glow"]
intensity = 20.0
"#,
    );
    scenes.insert_file(
        AssetId::from_u128(5),
        "garden",
        r#"
[assets]
house = { id = "00000000-0000-0000-0000-000000000004", path = "house.scene" }

[entities.home]
prefab = "house"

[changed."home/porch/light"."test::Glow"]
intensity = 5.0
"#,
    );
    let mut world = World::new();
    world.register::<Aim>();
    world.register::<Glow>();
    let scene = scenes
        .resolve(world.registry(), AssetId::from_u128(5))
        .expect("the garden");
    let root = world.spawn_scene("garden", &scene).expect("spawned");
    let home = world.find(&id("garden/home")).expect("the home");
    let texts = copied(&world, &[home]);
    let renaming = pasted(&texts, &mut world, "garden", &mut scenes);
    let light = world
        .find(&renaming.renamed(&id("garden/home/porch/light")))
        .expect("the copy's light");
    assert_eq!(world.get::<Glow>(light), Some(&Glow { intensity: 5.0 }));
    let text = file(&mut world, root);
    let new = renaming.renamed(&id("garden/home")).parts()[1].clone();
    assert!(
        text.contains(&format!(
            "[changed.\"{new}/porch/light\"]\n\"test::Glow\" = {{ intensity = 5.0 }}"
        )),
        "{text}"
    );
}

/// A renaming gives the identity that a command creates its own new one, and an identity that
/// extends one created the new one of the longest that it extends; it refuses commands that change
/// what they do not create, as a step of a journal pasted would.
#[test]
fn a_renaming_follows_the_longest_identity_created_and_refuses_what_changes_the_rest() {
    let (world, _, _) = yard();
    let place = |entity: &str| {
        format!(
            "[Place]\nentity = \"{entity}\"\nparent = \"yard\"\nprefab = \"00000000-0000-0000-0000-000000000002\"\n"
        )
    };
    let texts = [place("yard/shed"), place("yard/shed/lamp")];
    let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
    let renaming = Renaming::of_commands(&world, "yard", &texts).expect("renamed");
    let shed = renaming.renamed(&id("yard/shed"));
    let lamp = renaming.renamed(&id("yard/shed/lamp"));
    assert_eq!(
        lamp.parts().len(),
        2,
        "its own, not under the shed's: {lamp}"
    );
    assert!(lamp.to_string().starts_with("yard/lamp-"), "{lamp}");
    assert_eq!(
        renaming.renamed(&id("yard/shed/lamp/light")),
        PersistentId::new(lamp.parts().iter().map(String::as_str).chain(["light"]))
    );
    assert_eq!(
        renaming.renamed(&id("yard/shed/post")),
        PersistentId::new(shed.parts().iter().map(String::as_str).chain(["post"]))
    );
    assert_eq!(renaming.renamed(&id("yard/well")), id("yard/well"));
    // A deletion of what they created is theirs; of anything else, or a value set, is refused.
    let delete = |entity: &str| format!("[Delete]\nentity = \"{entity}\"\n");
    let ok = [place("yard/shed"), delete("yard/shed/light")];
    let ok: Vec<&str> = ok.iter().map(String::as_str).collect();
    assert!(Renaming::of_commands(&world, "yard", &ok).is_ok());
    for refused in [
        delete("yard/well"),
        "[SetProperty]\nentity = \"yard/well\"\ncomponent = \"Name\"\npath = \"0\"\nvalue = \"pit\"\n".to_owned(),
    ] {
        let texts = [place("yard/shed"), refused];
        let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
        let error = Renaming::of_commands(&world, "yard", &texts).expect_err("refused");
        assert!(error.to_string().contains("command 1"), "{error}");
    }
}

/// A resolved scene says which scenes it places, at any depth.
#[test]
fn a_scene_says_what_it_places() {
    let (world, mut scenes, _) = yard();
    let yard = scenes.resolve(world.registry(), YARD).expect("the yard");
    let lamp = scenes.resolve(world.registry(), LAMP).expect("the lamp");
    assert!(yard.places(LAMP));
    assert!(!lamp.places(YARD));
    assert!(!yard.places(YARD));
    scenes.insert_file(
        AssetId::from_u128(6),
        "street",
        r#"
[assets]
yard = { id = "00000000-0000-0000-0000-000000000001", path = "yard.scene" }

[entities.number_9]
prefab = "yard"
"#,
    );
    let street = scenes
        .resolve(world.registry(), AssetId::from_u128(6))
        .expect("the street");
    assert!(
        street.places(YARD) && street.places(LAMP),
        "within the yard too"
    );
}
