//! Saves loaded into a world made again: what they name, and what they hold that a world cannot
//! take (`docs/adr/0087-the-worlds-documents.md`,
//! `docs/adr/0093-saves-and-hot-reload-with-placements.md`).

use glam::{DVec3, Mat4};
use slime_assets::{AssetId, CookedScene, SceneNode};
use slime_reflect::Reflect;
use slime_world::{
    Component, DocumentError, Entity, Name, ResolvedScene, SceneLibrary, Transform, World,
};

/// What an entity aims at.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Aim")]
struct Aim {
    at: Entity,
}
impl Component for Aim {}

/// The entity whose persistent identity `id` writes.
fn entity(world: &World, id: &str) -> Entity {
    world
        .find(&id.parse().expect("an identity"))
        .unwrap_or_else(|| panic!("no `{id}`"))
}

/// A glTF scene named `name`, of nodes without parents named `nodes`, resolved.
fn cooked(name: &str, nodes: &[&str]) -> ResolvedScene {
    let scene = CookedScene {
        name: name.to_owned(),
        nodes: nodes
            .iter()
            .map(|node| SceneNode {
                name: (*node).to_owned(),
                parent: None,
                transform: Mat4::IDENTITY.to_cols_array(),
                meshes: Vec::new(),
                light: None,
                extras: Vec::new(),
                components: Vec::new(),
            })
            .collect(),
    };
    let id = AssetId::from_u128(name.len() as u128);
    ResolvedScene::from_cooked(id, &scene, World::new().registry()).expect("resolved")
}

#[test]
fn a_section_that_a_save_does_not_hold_is_a_warning_and_one_of_the_wrong_kind_refuses_it() {
    let start = || {
        let mut world = World::new();
        world
            .spawn_scene("a", &cooked("a", &["gate", "well"]))
            .expect("spawned");
        world
    };
    for (text, unknown) in [
        (
            "[chnaged.\"a/well\".Transform]\ntranslation = { x = 3.0 }\n",
            "chnaged",
        ),
        ("[removed]\nentity = [\"a/well\"]\n", "entity"),
        (
            "[scenes]\na = { name = \"a\", content = \"0\", contents = \"0\" }\n",
            "contents",
        ),
    ] {
        let loaded = start()
            .load(text, &mut SceneLibrary::new())
            .expect("a save");
        assert!(
            loaded.warnings.iter().any(|warning| warning
                .to_string()
                .contains(&format!("no field `{unknown}`"))),
            "{text}: {:?}",
            loaded.warnings
        );
    }
    for text in [
        "removed = \"a/well\"\n",
        "[created]\n\"created/1\" = 5\n",
        "changed = [1, 2]\n",
        "[added]\n\"a/well\" = \"Hidden\"\n",
    ] {
        let error = start()
            .load(text, &mut SceneLibrary::new())
            .expect_err(text);
        assert!(matches!(error, DocumentError::Read(_)), "{text}: {error}");
    }
}

/// A start world of two scenes, spawned in the order given, as a game that spawns its scenes from a
/// hash map, or as their loads end, may: the gate aims at the well.
fn start_in_order(a_first: bool) -> World {
    let mut world = World::new();
    world.register::<Aim>();
    let (a, b) = (
        cooked("a", &["gate", "well"]),
        cooked("bb", &["tower", "bell"]),
    );
    let order = if a_first {
        [("a", &a), ("b", &b)]
    } else {
        [("b", &b), ("a", &a)]
    };
    for (key, scene) in order {
        world.spawn_scene(key, scene).expect("spawned");
    }
    let (gate, well) = (entity(&world, "a/gate"), entity(&world, "a/well"));
    world.insert(gate, Aim { at: well });
    world
}

#[test]
fn a_reference_that_the_run_changed_is_saved_whatever_order_the_start_was_made_in() {
    // The run was made with `b` first, the start that the save compares with `a` first: the same
    // entities, under the same identities, in other slots.
    let mut run = start_in_order(false);
    let start = start_in_order(true);
    // The gate made to aim at the entity of the run that has the well's handle in the start.
    let target = entity(&start, "a/well");
    assert!(
        run.contains(target),
        "a live entity of the run in the well's slot of the start"
    );
    assert_ne!(run.persistent_id(target), start.persistent_id(target));
    run.insert(entity(&run, "a/gate"), Aim { at: target });
    let save = run.save(&start, &Default::default()).expect("saved");
    let mut loaded = start_in_order(true);
    loaded
        .load(&save, &mut SceneLibrary::new())
        .expect("loaded");
    assert_eq!(run.first_difference(&loaded), None, "{save}");
    // And a reference that the run left as it was is no difference.
    let unchanged = start_in_order(false)
        .save(&start, &Default::default())
        .expect("saved");
    assert!(!unchanged.contains("test::Aim"), "{unchanged}");
}

/// A start made in code, each of `names` an entity with that name and a transform: under its name
/// as a key if `keyed`, as created entities otherwise, in the order given.
fn made_in_code(names: &[&str], keyed: bool) -> World {
    let mut world = World::new();
    for &name in names {
        let entity = if keyed {
            world.spawn_keyed(name).expect("a free key")
        } else {
            world.spawn()
        };
        world.insert(entity, Name(name.to_owned()));
        world.insert(entity, Transform::IDENTITY);
    }
    world
}

/// The entity named `name`.
fn named(world: &World, name: &str) -> Entity {
    world
        .entities()
        .find(|&entity| world.get::<Name>(entity).is_some_and(|held| held.0 == name))
        .unwrap_or_else(|| panic!("no `{name}`"))
}

/// How far east the entity named `name` stands.
fn x(world: &World, name: &str) -> f64 {
    world
        .get::<Transform>(named(world, name))
        .expect("placed")
        .translation
        .x
}

#[test]
fn an_update_that_creates_one_more_entity_at_the_start_never_moves_another() {
    // Version 1 of a game makes the rig and the weather; the run moves the rig and creates an
    // enemy. Version 2 makes the music first.
    for keyed in [false, true] {
        let mut run = made_in_code(&["rig", "weather"], keyed);
        let rig = named(&run, "rig");
        run.insert(rig, Transform::from_translation(DVec3::new(5.0, 0.0, 0.0)));
        let enemy = run.spawn();
        run.insert(enemy, Name("enemy".to_owned()));
        let start = made_in_code(&["rig", "weather"], keyed);
        let save = run.save(&start, &Default::default()).expect("saved");
        let mut loaded = made_in_code(&["music", "rig", "weather"], keyed);
        let report = loaded
            .load(&save, &mut SceneLibrary::new())
            .expect("the save of a run loads after an update");
        assert_eq!(x(&loaded, "music"), 0.0, "keyed: {keyed}");
        // What the run created takes the numbers after the new start's.
        assert_eq!(
            loaded.persistent_id(named(&loaded, "enemy")),
            Some(
                if keyed { "created/1" } else { "created/4" }
                    .parse()
                    .unwrap()
            )
        );
        if keyed {
            // A key names the rig whatever the start makes.
            assert_eq!(x(&loaded, "rig"), 5.0);
            assert!(report.set_aside.is_empty(), "{report:?}");
        } else {
            // A number may name another entity: the move is set aside, and said.
            assert_eq!(x(&loaded, "rig"), 0.0);
            assert_eq!(report.set_aside.len(), 1, "{report:?}");
            assert!(
                report.set_aside[0].reason.contains("spawn_keyed"),
                "{report:?}"
            );
        }
    }
    // The same start, made in another order: a number names another entity, a key the same one.
    let mut run = made_in_code(&["rig", "weather"], false);
    let rig = named(&run, "rig");
    run.insert(rig, Transform::from_translation(DVec3::new(5.0, 0.0, 0.0)));
    let save = run
        .save(
            &made_in_code(&["rig", "weather"], false),
            &Default::default(),
        )
        .expect("saved");
    let mut swapped = made_in_code(&["weather", "rig"], false);
    let report = swapped
        .load(&save, &mut SceneLibrary::new())
        .expect("loaded");
    assert_eq!((x(&swapped, "rig"), x(&swapped, "weather")), (0.0, 0.0));
    assert_eq!(report.set_aside.len(), 1, "{report:?}");
}

#[test]
fn an_entity_that_a_run_creates_under_a_key_is_saved_and_loads_back() {
    let start = made_in_code(&["rig"], true);
    let mut run = made_in_code(&["rig"], true);
    let boss = run.spawn_keyed("boss").expect("a free key");
    run.insert(boss, Name("boss".to_owned()));
    run.set_parent(boss, Some(named(&run, "rig")))
        .expect("alive");
    let save = run.save(&start, &Default::default()).expect("saved");
    assert!(save.contains("[created.boss"), "{save}");
    let mut loaded = made_in_code(&["rig"], true);
    loaded
        .load(&save, &mut SceneLibrary::new())
        .expect("loaded");
    assert_eq!(run.first_difference(&loaded), None);
    // A start that holds the key already.
    let mut taken = made_in_code(&["rig", "boss"], true);
    let error = taken
        .load(&save, &mut SceneLibrary::new())
        .expect_err("in use");
    assert!(
        matches!(error, DocumentError::IdentityInUse(ref id) if id.to_string() == "boss"),
        "{error}"
    );
}

#[test]
fn a_hot_reload_after_a_level_gains_a_marker_moves_only_the_enemy_that_the_run_moved() {
    const LEVEL: AssetId = AssetId::from_u128(10);
    const ENEMY: AssetId = AssetId::from_u128(11);
    let library = |level: &str| {
        let mut scenes = SceneLibrary::new();
        scenes.insert_file(
            ENEMY,
            "enemy",
            "[entities.body.components]\nName = \"body\"\n",
        );
        scenes.insert_file(LEVEL, "level", level.to_owned());
        scenes
    };
    let marker = |name: &str, x: f64| {
        format!(
            "[entities.\"{name}\".components]\nName = \"{name}\"\n\
             Transform = {{ translation = {{ x = {x:?}, y = 0.0, z = 0.0 }} }}\n"
        )
    };
    let before = format!("{}{}", marker("b", 20.0), marker("c", 30.0));
    let after = format!("{}{before}", marker("a", 10.0));
    // The game's start places an enemy at each marker, in the level's order: under a key named
    // after the marker if `keyed`, as a placement made while the world runs otherwise.
    let start = |scenes: &mut SceneLibrary, keyed: bool| {
        let mut world = World::new();
        let level = scenes.resolve(world.registry(), LEVEL).expect("resolved");
        let root = world.spawn_scene("level", &level).expect("spawned");
        let enemy = scenes.resolve(world.registry(), ENEMY).expect("resolved");
        for marker in world.children(root).to_vec() {
            let name = format!("enemy at {}", world.get::<Name>(marker).expect("named").0);
            let at = world.get::<Transform>(marker).expect("placed").translation;
            let placed = if keyed {
                world.spawn_scene(&name, &enemy).expect("spawned")
            } else {
                world.spawn_placement(&enemy, None).expect("placed")
            };
            world.insert(placed, Name(name));
            world.insert(placed, Transform::from_translation(at));
        }
        world
    };
    for keyed in [false, true] {
        let mut old = library(&before);
        let mut run = start(&mut old, keyed);
        let enemy = named(&run, "enemy at b");
        run.insert(
            enemy,
            Transform::from_translation(DVec3::new(100.0, 0.0, 0.0)),
        );
        let old_start = start(&mut old, keyed);
        let mut new = library(&after);
        let new_start = start(&mut new, keyed);
        let (carried, report) = run.carry(&old_start, new_start, &mut new).expect("carried");
        assert_eq!(x(&carried, "enemy at a"), 10.0, "keyed: {keyed}");
        assert_eq!(x(&carried, "enemy at c"), 30.0, "keyed: {keyed}");
        let moved = x(&carried, "enemy at b");
        if keyed {
            assert_eq!(moved, 100.0);
        } else {
            assert_eq!(moved, 20.0, "set aside rather than given to another enemy");
            assert!(!report.set_aside.is_empty());
        }
    }
}
