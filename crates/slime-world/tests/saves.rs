//! Saves loaded into a world made again: what they name, and what they hold that a world cannot
//! take (`docs/adr/0087-the-worlds-documents.md`,
//! `docs/adr/0093-saves-and-hot-reload-with-placements.md`).

use glam::{DVec3, Mat4};
use slime_assets::{AssetId, CookedScene, SceneNode};
use slime_reflect::Reflect;
use slime_world::{
    Component, DocumentError, Driven, Entity, Hidden, Name, ResolvedScene, SceneLibrary, Transform,
    Unsaved, World, WorldError,
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
fn what_a_run_changed_of_the_start_made_in_code_loads_back_into_the_same_start() {
    let mut run = made_in_code(&["rig", "weather"], true);
    let rig = named(&run, "rig");
    run.insert(rig, Transform::from_translation(DVec3::new(5.0, 0.0, 0.0)));
    let start = made_in_code(&["rig", "weather"], true);
    let save = run.save(&start, &Default::default()).expect("saved");
    let mut loaded = made_in_code(&["rig", "weather"], true);
    let report = loaded
        .load(&save, &mut SceneLibrary::new())
        .expect("loaded");
    assert!(report.set_aside.is_empty(), "{report:?}");
    assert_eq!(run.first_difference(&loaded), None);
}

#[test]
fn a_start_that_created_entities_without_a_key_refuses_to_save_naming_them() {
    let start = made_in_code(&["rig", "weather"], false);
    let refused = start
        .save(
            &made_in_code(&["rig", "weather"], false),
            &Default::default(),
        )
        .expect_err("refused");
    assert!(
        matches!(&refused, DocumentError::UnkeyedAtStart(named)
            if named == &["`created/1` (rig)".to_owned(), "`created/2` (weather)".to_owned()]),
        "{refused}"
    );
    assert!(refused.to_string().contains("spawn_keyed"), "{refused}");
    // What saves leave out needs no key: the run's own creations neither.
    let unsaved = || {
        let mut world = made_in_code(&["rig"], true);
        let spark = world.spawn();
        world.insert(spark, Unsaved);
        world
    };
    let mut run = unsaved();
    run.spawn();
    assert!(run.save(&unsaved(), &Default::default()).is_ok());
}

#[test]
fn an_update_that_creates_one_more_entity_at_the_start_never_moves_another() {
    // Version 1 of a game makes the rig and the weather; the run moves the rig and creates an
    // enemy. Version 2 makes the music first: a key names the rig whatever the start makes. Saves
    // made before saving refused a start without keys are read by its fingerprint (the tests of
    // `document.rs`).
    let mut run = made_in_code(&["rig", "weather"], true);
    let rig = named(&run, "rig");
    run.insert(rig, Transform::from_translation(DVec3::new(5.0, 0.0, 0.0)));
    let enemy = run.spawn();
    run.insert(enemy, Name("enemy".to_owned()));
    let start = made_in_code(&["rig", "weather"], true);
    let save = run.save(&start, &Default::default()).expect("saved");
    let mut loaded = made_in_code(&["music", "rig", "weather"], true);
    let report = loaded
        .load(&save, &mut SceneLibrary::new())
        .expect("the save of a run loads after an update");
    assert_eq!(x(&loaded, "music"), 0.0);
    assert_eq!(x(&loaded, "rig"), 5.0);
    assert!(report.set_aside.is_empty(), "{report:?}");
    // What the run created takes the numbers after the new start's.
    assert_eq!(
        loaded.persistent_id(named(&loaded, "enemy")),
        Some("created/1".parse().unwrap())
    );
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
    // The game's start places an enemy at each marker, in the level's order, under a key named
    // after the marker. Placed without a key, as saves made before saving refused it name them, a
    // number would name another enemy once the level gains a marker (the tests of `document.rs`).
    let start = |scenes: &mut SceneLibrary| {
        let mut world = World::new();
        let level = scenes.resolve(world.registry(), LEVEL).expect("resolved");
        let root = world.spawn_scene("level", &level).expect("spawned");
        let enemy = scenes.resolve(world.registry(), ENEMY).expect("resolved");
        for marker in world.children(root).collect::<Vec<_>>() {
            let name = format!("enemy at {}", world.get::<Name>(marker).expect("named").0);
            let at = world.get::<Transform>(marker).expect("placed").translation;
            let placed = world.spawn_scene(&name, &enemy).expect("spawned");
            world.insert(placed, Name(name));
            world.insert(placed, Transform::from_translation(at));
        }
        world
    };
    let mut old = library(&before);
    let mut run = start(&mut old);
    let enemy = named(&run, "enemy at b");
    run.insert(
        enemy,
        Transform::from_translation(DVec3::new(100.0, 0.0, 0.0)),
    );
    let old_start = start(&mut old);
    let mut new = library(&after);
    let new_start = start(&mut new);
    let (carried, report) = run.carry(&old_start, new_start, &mut new).expect("carried");
    assert_eq!(x(&carried, "enemy at a"), 10.0);
    assert_eq!(x(&carried, "enemy at c"), 30.0);
    assert_eq!(x(&carried, "enemy at b"), 100.0);
    assert!(report.set_aside.is_empty(), "{report:?}");
}

/// The levels of a run that changes level under one key, `level`, and the hero's prefab.
const TOWN: AssetId = AssetId::from_u128(20);
const CAVE: AssetId = AssetId::from_u128(21);
const HERO: AssetId = AssetId::from_u128(22);

/// The town, a floor, a marker where the hero starts and a companion; the cave, a rock; the hero, a
/// sword.
fn levels() -> SceneLibrary {
    let mut scenes = SceneLibrary::new();
    let entities = |names: &[&str]| {
        names
            .iter()
            .map(|name| format!("[entities.{name}.components]\nName = \"{name}\"\n"))
            .collect::<String>()
    };
    scenes.insert_file(TOWN, "town", entities(&["floor", "start", "companion"]));
    scenes.insert_file(CAVE, "cave", entities(&["rock"]));
    scenes.insert_file(HERO, "hero", entities(&["sword"]));
    scenes
}

/// The start of a run: the town under `level`, and the hero, created outside scenes under a key.
fn in_town(scenes: &mut SceneLibrary) -> World {
    let mut world = World::new();
    let town = scenes.resolve(world.registry(), TOWN).expect("resolved");
    world.spawn_scene("level", &town).expect("spawned");
    let hero = world.spawn_keyed("hero").expect("a free key");
    world.insert(hero, Name("hero".to_owned()));
    world
}

/// The town of `world`, under `key`, left for the cave under `level`.
fn to_the_cave(world: &mut World, key: &str, scenes: &mut SceneLibrary) {
    world.despawn(entity(world, key));
    let cave = scenes.resolve(world.registry(), CAVE).expect("resolved");
    world.spawn_scene("level", &cave).expect("spawned");
}

#[test]
fn a_level_spawned_under_a_key_waits_until_nothing_of_the_one_before_lives() {
    let mut scenes = levels();
    let mut run = in_town(&mut scenes);
    // The companion follows the hero out of the town, whose root is destroyed.
    let hero = entity(&run, "hero");
    run.set_parent(entity(&run, "level/companion"), Some(hero))
        .expect("alive");
    run.despawn(entity(&run, "level"));
    let cave = scenes.resolve(run.registry(), CAVE).expect("resolved");
    let refused = run
        .spawn_scene("level", &cave)
        .expect_err("the companion lives");
    assert_eq!(
        refused,
        WorldError::KeyInUse {
            key: "level".to_owned(),
            entity: "level/companion".parse().unwrap(),
        }
    );
    // The cave under a key of its own keeps the companion, through a save and a load.
    run.spawn_scene("cave", &cave).expect("spawned");
    let save = run
        .save(&in_town(&mut scenes), &Default::default())
        .expect("saved");
    let mut loaded = in_town(&mut scenes);
    let report = loaded.load(&save, &mut scenes).expect("loaded");
    assert!(report.set_aside.is_empty(), "{report:?}");
    assert_eq!(run.first_difference(&loaded), None);
    // Once the companion is gone, the cave goes under `level`.
    let mut run = in_town(&mut scenes);
    to_the_cave(&mut run, "level", &mut scenes);
    assert!(run.find(&"level/rock".parse().unwrap()).is_some());
}

#[test]
fn what_the_run_placed_in_a_level_outlives_the_run_leaving_it() {
    let mut scenes = levels();
    // The game's start spawns the town; the run places the hero's prefab at the town's start
    // marker, as a game spawns its player at a spawn point. A start that placed it would have to
    // give it a key, as the next test does.
    let start = |scenes: &mut SceneLibrary| {
        let mut world = World::new();
        let town = scenes.resolve(world.registry(), TOWN).expect("resolved");
        world.spawn_scene("level", &town).expect("spawned");
        world
    };
    let arrive = |world: &mut World, scenes: &mut SceneLibrary| {
        let hero = scenes.resolve(world.registry(), HERO).expect("resolved");
        let marker = entity(world, "level/start");
        world.spawn_placement(&hero, Some(marker)).expect("placed");
    };
    // The hero walks out of the town, which gives way to the cave.
    let mut run = start(&mut scenes);
    arrive(&mut run, &mut scenes);
    run.set_parent(entity(&run, "created/1"), None)
        .expect("alive");
    to_the_cave(&mut run, "level", &mut scenes);
    let save = run
        .save(&start(&mut scenes), &Default::default())
        .expect("saved");
    let mut loaded = start(&mut scenes);
    let report = loaded.load(&save, &mut scenes).expect("loaded");
    assert!(report.set_aside.is_empty(), "{report:?}");
    assert!(loaded.find(&"created/1/sword".parse().unwrap()).is_some());
    assert_eq!(run.first_difference(&loaded), None);
    // Loaded, then saved again: the same save.
    let again = loaded
        .save(&start(&mut scenes), &Default::default())
        .expect("saved");
    assert_eq!(again, save);
    // Carried through hot reload, the cave edited: the edit reaches the run, the hero stays.
    scenes.insert_file(
        CAVE,
        "cave",
        "[entities.rock.components]\nName = \"rock\"\n\
         [entities.torch.components]\nName = \"torch\"\n",
    );
    let (carried, report) = run
        .carry(&start(&mut levels()), start(&mut scenes), &mut scenes)
        .expect("carried");
    assert!(report.set_aside.is_empty(), "{report:?}");
    assert!(carried.find(&"level/torch".parse().unwrap()).is_some());
    assert!(carried.find(&"created/1/sword".parse().unwrap()).is_some());
    // A hero that stayed in the town is gone with it, in the run and once loaded.
    let mut scenes = levels();
    let mut run = start(&mut scenes);
    arrive(&mut run, &mut scenes);
    to_the_cave(&mut run, "level", &mut scenes);
    let save = run
        .save(&start(&mut scenes), &Default::default())
        .expect("saved");
    let mut loaded = start(&mut scenes);
    let report = loaded.load(&save, &mut scenes).expect("loaded");
    assert!(report.set_aside.is_empty(), "{report:?}");
    assert!(loaded.find(&"created/1".parse().unwrap()).is_none());
    assert_eq!(run.first_difference(&loaded), None);
}

#[test]
fn a_scene_that_the_start_places_in_a_level_outlives_the_run_leaving_it() {
    let mut scenes = levels();
    // The game's start spawns the hero under its own key, on the town's floor.
    let start = |scenes: &mut SceneLibrary| {
        let mut world = World::new();
        let town = scenes.resolve(world.registry(), TOWN).expect("resolved");
        world.spawn_scene("level", &town).expect("spawned");
        let hero = scenes.resolve(world.registry(), HERO).expect("resolved");
        let hero = world.spawn_scene("hero", &hero).expect("spawned");
        let floor = entity(&world, "level/floor");
        world.set_parent(hero, Some(floor)).expect("alive");
        world
    };
    let mut run = start(&mut scenes);
    run.set_parent(entity(&run, "hero"), None).expect("alive");
    to_the_cave(&mut run, "level", &mut scenes);
    let save = run
        .save(&start(&mut scenes), &Default::default())
        .expect("saved");
    let mut loaded = start(&mut scenes);
    let report = loaded.load(&save, &mut scenes).expect("loaded");
    assert!(report.set_aside.is_empty(), "{report:?}");
    assert!(loaded.find(&"hero/sword".parse().unwrap()).is_some());
    assert_eq!(run.first_difference(&loaded), None);
}

#[test]
fn a_component_of_a_type_or_a_variant_that_this_program_does_not_know_is_a_warning() {
    // As a later version of a game writes them: a type of its own, and a variant that it added.
    let mut world = World::new();
    world
        .spawn_scene("a", &cooked("a", &["gate", "well"]))
        .expect("spawned");
    let text = "[added.\"a/gate\"]\n\"test::Chest\" = { gold = 3 }\n\
                [added.\"a/well\"]\nLight = { Laser = { color = [1.0, 0.0, 0.0, 1.0] } }\n\
                [changed.\"a/well\".Transform]\ntranslation = { x = -1.0 }\n";
    let loaded = world.load(text, &mut SceneLibrary::new()).expect("a save");
    assert!(loaded.set_aside.is_empty(), "{:?}", loaded.set_aside);
    let warnings: Vec<String> = loaded.warnings.iter().map(ToString::to_string).collect();
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings[0].contains("test::Chest"), "{warnings:?}");
    assert!(warnings[1].contains("Laser"), "{warnings:?}");
    // The rest is applied: the well's transform.
    let well = entity(&world, "a/well");
    assert_eq!(
        world.get::<Transform>(well).map(|at| at.translation.x),
        Some(-1.0)
    );
}

#[test]
fn a_save_that_gives_a_key_to_a_scene_and_to_an_entity_it_creates_is_refused() {
    // Edited by hand: the run spawned a scene under `camp`, and created an entity under the same key.
    let text = "[spawned.camp]\nscene = \"00000000-0000-0000-0000-000000000007\"\n\
                name = \"camp\"\ncontent = \"0000000000000000\"\n\
                [created.camp]\nparent = []\n";
    let mut world = World::new();
    let error = world
        .load(text, &mut SceneLibrary::new())
        .expect_err("one identity, two entities");
    assert!(
        matches!(&error, DocumentError::IdentityInUse(id) if id.to_string() == "camp"),
        "{error}"
    );
    assert!(world.is_empty(), "the world is left as it was");
}

#[test]
fn a_save_that_holds_many_unknown_keys_warns_of_the_first_and_counts_the_rest() {
    let keys: String = (0..1000)
        .map(|index| format!("unknown{index} = 1\n"))
        .collect();
    let mut world = World::new();
    let loaded = world.load(&keys, &mut SceneLibrary::new()).expect("a save");
    assert_eq!(loaded.warnings.len(), slime_serialize::MAX_WARNINGS + 1);
    assert!(
        loaded.warnings[0].to_string().contains("unknown0"),
        "{}",
        loaded.warnings[0]
    );
    let more = loaded.warnings.last().expect("a last warning");
    assert!(more.to_string().starts_with("900 more"), "{more}");
}

#[test]
fn a_save_that_spawns_more_bytes_of_scenes_than_a_scene_holds_sets_the_rest_aside() {
    // A scene of 2 MiB, which a save edited by hand spawns 130 times: 260 MiB, with far fewer
    // entities than the limit.
    let big = AssetId::from_u128(7);
    let mut scenes = SceneLibrary::new();
    scenes.insert_file(
        big,
        "big",
        format!("[root.components]\nName = \"{}\"\n", "a".repeat(2 << 20)),
    );
    let text: String = (0..130)
        .map(|index| {
            format!(
                "[spawned.k{index}]\nscene = \"{big}\"\nname = \"big\"\n\
                 content = \"0000000000000000\"\n\n"
            )
        })
        .collect();
    let mut world = World::new();
    let loaded = world.load(&text, &mut scenes).expect("a save");
    let aside: Vec<&str> = loaded
        .set_aside
        .iter()
        .map(|aside| aside.reason.as_str())
        .collect();
    assert!((1..10).contains(&aside.len()), "{aside:?}");
    assert!(
        aside
            .iter()
            .all(|reason| reason.contains("bytes of scenes")),
        "{aside:?}"
    );
}

#[test]
fn a_save_that_spawns_a_scene_under_a_long_key_counts_it_in_the_limit() {
    // A scene of 1 000 entities, which a save edited by hand spawns under a key of 4 MiB that each
    // entity's identity repeats: 8 GB, from a save of 4 MiB.
    let lamp = AssetId::from_u128(7);
    let entities: String = (0..1_000)
        .map(|index| format!("[entities.e{index}.components]\nName = \"e{index}\"\n\n"))
        .collect();
    let mut scenes = SceneLibrary::new();
    scenes.insert_file(lamp, "lamp", entities);
    let text = format!(
        "[spawned.{}]\nscene = \"{lamp}\"\nname = \"lamp\"\ncontent = \"0000000000000000\"\n",
        "a".repeat(4 << 20)
    );
    let mut world = World::new();
    let loaded = world.load(&text, &mut scenes).expect("a save");
    assert!(
        loaded.set_aside.len() == 1 && loaded.set_aside[0].reason.contains("bytes of scenes"),
        "{:?}",
        loaded
            .set_aside
            .iter()
            .map(|aside| &aside.reason)
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_created_entity_written_into_its_level_then_carried_exists_once() {
    // A run creates a torch under the zone, and the well aims at it; the zone is written back, as
    // an editor saves a level, and the file cooked again; the run is carried into a start made
    // from the new file, as hot reload does (the audit of 0.4 found the torch there twice).
    const ZONE: AssetId = AssetId::from_u128(30);
    let mut library = SceneLibrary::new();
    library.insert_file(
        ZONE,
        "zone",
        "[entities.well.components]\nName = \"well\"\n",
    );
    let start = |library: &mut SceneLibrary| {
        let mut world = World::new();
        world.register::<Aim>();
        let scene = library.resolve(world.registry(), ZONE).expect("resolves");
        world.spawn_scene("zone", &scene).expect("spawns");
        world
    };
    let old_start = start(&mut library);
    let mut run = start(&mut library);
    let zone = entity(&run, "zone");
    let torch = run.spawn();
    run.insert(torch, Name("torch".to_owned()));
    run.insert(torch, Transform::IDENTITY);
    run.set_parent(torch, Some(zone)).expect("both alive");
    let well = entity(&run, "zone/well");
    run.insert(well, Aim { at: torch });
    let written = run.write_scene(zone).expect("written");
    // After the file is written, the run moves the torch on.
    run.insert(
        torch,
        Transform::from_translation(DVec3::new(2.0, 0.0, 0.0)),
    );
    let mut cooked = SceneLibrary::new();
    cooked.insert_file(ZONE, "zone", written.text.clone());
    let (carried, report) = run
        .carry(&old_start, start(&mut cooked), &mut cooked)
        .expect("carried");
    assert!(report.set_aside.is_empty(), "{report:?}");
    let torches: Vec<Entity> = carried
        .entities()
        .filter(|&entity| {
            carried
                .get::<Name>(entity)
                .is_some_and(|name| name.0 == "torch")
        })
        .collect();
    assert_eq!(torches.len(), 1, "the torch exists once");
    let torch = torches[0];
    let id = carried
        .persistent_id(torch)
        .expect("an identity")
        .to_string();
    assert!(id.starts_with("zone/torch-"), "the file's: {id}");
    assert_eq!(x(&carried, "torch"), 2.0, "the run's move kept");
    assert_eq!(
        carried.get::<Aim>(entity(&carried, "zone/well")),
        Some(&Aim { at: torch }),
        "the well aims at the file's torch"
    );
    // A run carried into the file that is not cooked again creates it as before.
    let (again, report) = run
        .carry(&old_start, start(&mut library), &mut library)
        .expect("carried");
    assert!(report.set_aside.is_empty(), "{report:?}");
    assert!(again.find(&"created/1".parse().unwrap()).is_some());
}

#[test]
fn a_driven_component_is_saved_and_written_back_as_its_author_wrote_it() {
    // A level whose ring floats, its transform driven, and a lamp of a prefab whose transform the
    // level overrides and drives, as a game moves them at each step; the level adds to the lamp's
    // light an aim at the ring, which it drives with the light's transform, the prefab's.
    const LAMP: AssetId = AssetId::from_u128(31);
    const LEVEL: AssetId = AssetId::from_u128(32);
    let mut scenes = SceneLibrary::new();
    let lamp_file = "[entities.light.components]\nName = \"light\"\n\
                Transform = { translation = { x = 0.0, y = 1.0, z = 0.0 } }\n";
    scenes.insert_file(LAMP, "lamp", lamp_file);
    let level = format!(
        "[assets]\nlamp = {{ id = \"{LAMP}\", path = \"lamp.scene\" }}\n\n\
         [entities.crate.components]\nName = \"crate\"\n\
         Transform = {{ translation = {{ x = 2.0, y = 0.0, z = 0.0 }} }}\n\n\
         [entities.barrel.components]\nName = \"barrel\"\nDriven = [\"Transform\"]\n\
         Transform = {{ translation = {{ x = 5.0, y = 0.0, z = 0.0 }} }}\n\n\
         [entities.ring.components]\nName = \"ring\"\nDriven = [\"Transform\"]\n\
         Transform = {{ translation = {{ x = 1.0, y = 0.0, z = 0.0 }} }}\n\n\
         [entities.lamp]\nprefab = \"lamp\"\n\n\
         [changed.lamp.Transform]\ntranslation = {{ x = 3.0 }}\n\n\
         [added.lamp]\nDriven = [\"Transform\"]\n\n\
         [added.\"lamp/light\"]\nDriven = [\"Transform\", \"test::Aim\"]\n\
         \"test::Aim\" = {{ at = \"ring\" }}\n"
    );
    scenes.insert_file(LEVEL, "level", level);
    let start = |scenes: &mut SceneLibrary| {
        let mut world = World::new();
        world.register::<Aim>();
        let level = scenes.resolve(world.registry(), LEVEL).expect("resolved");
        world.spawn_scene("level", &level).expect("spawned");
        world
    };
    let moved = |x: f64| Transform::from_translation(DVec3::new(x, 0.0, 0.0));
    let mut run = start(&mut scenes);
    run.insert(entity(&run, "level/ring"), moved(1.25));
    run.insert(entity(&run, "level/lamp"), moved(3.5));
    let light = entity(&run, "level/lamp/light");
    run.insert(light, moved(0.5));
    let crate_at = entity(&run, "level/crate");
    run.insert(light, Aim { at: crate_at });
    // Physics takes the crate over, and the run marks its transform driven: the mark that its
    // author wrote governs the save, so where it fell is saved.
    let crate_ = entity(&run, "level/crate");
    run.insert(crate_, moved(9.0));
    run.insert(crate_, Driven(vec!["Transform".to_owned()]));
    // The barrel settles, and the run takes its mark off: where the run left it is saved too.
    let barrel = entity(&run, "level/barrel");
    run.insert(barrel, moved(6.0));
    run.remove::<Driven>(barrel);
    // A spark that the run creates in the level and drives: no author wrote it, so it is saved and
    // written as it is.
    let spark = run.spawn();
    run.insert(spark, Name("spark".to_owned()));
    run.insert(spark, moved(7.0));
    run.insert(spark, Driven(vec!["Transform".to_owned()]));
    run.set_parent(spark, Some(entity(&run, "level")))
        .expect("both alive");

    let save = run
        .save(&start(&mut scenes), &Default::default())
        .expect("saved");
    assert!(!save.contains("level/ring"), "{save}");
    assert!(!save.contains("level/lamp"), "{save}");
    let mut loaded = start(&mut scenes);
    let report = loaded.load(&save, &mut scenes).expect("loaded");
    assert!(report.set_aside.is_empty(), "{report:?}");
    assert_eq!(
        (
            x(&loaded, "ring"),
            x(&loaded, "spark"),
            x(&loaded, "crate"),
            x(&loaded, "barrel")
        ),
        (1.0, 7.0, 9.0, 6.0)
    );
    let lamp = entity(&loaded, "level/lamp");
    assert_eq!(
        loaded.get::<Transform>(lamp).expect("placed").translation.x,
        3.0
    );
    // What a system computes again after a load is no difference.
    assert_eq!(run.first_difference(&loaded), None);

    // Written back as an editor saves a level: as the author wrote it.
    let root = entity(&run, "level");
    let written = run.write_scene(root).expect("written");
    let mut again = SceneLibrary::new();
    again.insert_file(LAMP, "lamp", lamp_file);
    again.insert_file(LEVEL, "level", written.text.clone());
    let spawned = start(&mut again);
    assert_eq!(x(&spawned, "ring"), 1.0, "{}", written.text);
    let lamp = entity(&spawned, "level/lamp");
    assert_eq!(
        spawned
            .get::<Transform>(lamp)
            .expect("placed")
            .translation
            .x,
        3.0,
        "{}",
        written.text
    );
    assert_eq!(x(&spawned, "spark"), 7.0, "{}", written.text);
    // The light: its aim as the level added it, its transform as the prefab gives it.
    let light = entity(&spawned, "level/lamp/light");
    assert_eq!(
        spawned.get::<Aim>(light),
        Some(&Aim {
            at: entity(&spawned, "level/ring")
        }),
        "{}",
        written.text
    );
    assert!(
        !written.text.contains("[changed.\"lamp/light\".Transform]"),
        "{}",
        written.text
    );
}

/// The zone of the torch's tests, with a well, and a lamp prefab that a run places in it.
const TORCH_ZONE: AssetId = AssetId::from_u128(40);
const TORCH_LAMP: AssetId = AssetId::from_u128(41);
const TORCH_CAVE: AssetId = AssetId::from_u128(42);

fn torch_scenes() -> SceneLibrary {
    let mut scenes = SceneLibrary::new();
    scenes.insert_file(
        TORCH_ZONE,
        "zone",
        "[entities.well.components]\nName = \"well\"\n",
    );
    scenes.insert_file(
        TORCH_CAVE,
        "cave",
        "[entities.rock.components]\nName = \"rock\"\n",
    );
    scenes.insert_file(
        TORCH_LAMP,
        "lamp",
        "[entities.light.components]\nName = \"light\"\n\
         [entities.glass.components]\nName = \"glass\"\n",
    );
    scenes
}

/// A start of the torch's tests: the zone under `zone`.
fn zone_start(scenes: &mut SceneLibrary) -> World {
    let mut world = World::new();
    let zone = scenes
        .resolve(world.registry(), TORCH_ZONE)
        .expect("resolved");
    world.spawn_scene("zone", &zone).expect("spawned");
    world
}

/// The entities named `name`.
fn all_named(world: &World, name: &str) -> Vec<Entity> {
    world
        .entities()
        .filter(|&entity| world.get::<Name>(entity).is_some_and(|held| held.0 == name))
        .collect()
}

#[test]
fn a_placement_made_while_running_written_into_its_level_then_carried_exists_once() {
    let mut scenes = torch_scenes();
    let old_start = zone_start(&mut scenes);
    let mut run = zone_start(&mut scenes);
    let lamp = scenes
        .resolve(run.registry(), TORCH_LAMP)
        .expect("resolved");
    let zone = entity(&run, "zone");
    let placed = run.spawn_placement(&lamp, Some(zone)).expect("placed");
    // Before the file is written, the run takes the lamp's glass out, renames its light and hides
    // it: the file holds them as its overrides.
    run.despawn(entity(&run, "created/1/glass"));
    let light = entity(&run, "created/1/light");
    run.insert(light, Name("light, lit".to_owned()));
    run.insert(placed, Hidden);
    let written = run.write_scene(zone).expect("written");
    // After, the run moves the lamp, names its light back as the prefab does, and shows it again:
    // the run's values win over the file's overrides.
    run.insert(
        placed,
        Transform::from_translation(DVec3::new(4.0, 0.0, 0.0)),
    );
    run.insert(light, Name("light".to_owned()));
    run.remove::<Hidden>(placed);
    let mut cooked = torch_scenes();
    cooked.insert_file(TORCH_ZONE, "zone", written.text);
    let (carried, report) = run
        .carry(&old_start, zone_start(&mut cooked), &mut cooked)
        .expect("carried");
    assert!(report.set_aside.is_empty(), "{report:?}");
    let lamps: Vec<Entity> = carried
        .entities()
        .filter(|&entity| carried.placement(entity).is_some())
        .collect();
    assert_eq!(lamps.len(), 1, "the lamp is placed once");
    let id = carried
        .persistent_id(lamps[0])
        .expect("an identity")
        .to_string();
    assert!(id.starts_with("zone/lamp-"), "the file's: {id}");
    let at = carried
        .get::<Transform>(lamps[0])
        .expect("placed")
        .translation;
    assert_eq!(at.x, 4.0, "the run's move kept");
    assert!(carried.get::<Hidden>(lamps[0]).is_none(), "shown again");
    assert_eq!(
        all_named(&carried, "light").len(),
        1,
        "named as the run names it"
    );
    assert!(all_named(&carried, "light, lit").is_empty());
    assert!(all_named(&carried, "glass").is_empty(), "taken out before");
}

#[test]
fn a_created_entity_written_into_a_level_that_the_run_changed_to_exists_once() {
    // The run leaves the zone for the cave, under a key of its own, creates a torch in the cave
    // and writes the cave back; carried once the cave is cooked again, the torch is the file's.
    let mut scenes = torch_scenes();
    let old_start = zone_start(&mut scenes);
    let mut run = zone_start(&mut scenes);
    let cave = scenes
        .resolve(run.registry(), TORCH_CAVE)
        .expect("resolved");
    let cave = run.spawn_scene("cave", &cave).expect("spawned");
    let torch = run.spawn();
    run.insert(torch, Name("torch".to_owned()));
    run.set_parent(torch, Some(cave)).expect("both alive");
    // A child that the run creates under the torch after the file is written follows it.
    let written = run.write_scene(cave).expect("written");
    let flame = run.spawn();
    run.insert(flame, Name("flame".to_owned()));
    run.set_parent(flame, Some(torch)).expect("both alive");
    let mut cooked = torch_scenes();
    cooked.insert_file(TORCH_CAVE, "cave", written.text);
    let (carried, report) = run
        .carry(&old_start, zone_start(&mut cooked), &mut cooked)
        .expect("carried");
    assert!(report.set_aside.is_empty(), "{report:?}");
    let torches = all_named(&carried, "torch");
    assert_eq!(torches.len(), 1, "the torch exists once");
    let id = carried
        .persistent_id(torches[0])
        .expect("an identity")
        .to_string();
    assert!(id.starts_with("cave/torch-"), "the file's: {id}");
    let flames = all_named(&carried, "flame");
    assert_eq!(flames.len(), 1);
    assert_eq!(carried.parent(flames[0]), Some(torches[0]));
}

#[test]
fn the_identity_that_a_file_gives_a_created_entity_outlives_a_load_before_the_file_is_cooked() {
    let mut scenes = torch_scenes();
    let old_start = zone_start(&mut scenes);
    let mut run = zone_start(&mut scenes);
    let zone = entity(&run, "zone");
    let torch = run.spawn();
    run.insert(torch, Name("torch".to_owned()));
    run.insert(torch, Transform::IDENTITY);
    run.set_parent(torch, Some(zone)).expect("both alive");
    let written = run.write_scene(zone).expect("written");
    // Saved and resumed before the file is cooked: the torch is created again, and remembers
    // the file's identity, which a file written again gives it again.
    let save = run.save(&old_start, &Default::default()).expect("saved");
    let mut resumed = zone_start(&mut scenes);
    resumed.load(&save, &mut scenes).expect("loaded");
    assert_eq!(all_named(&resumed, "torch").len(), 1);
    let rewritten = resumed
        .write_scene(entity(&resumed, "zone"))
        .expect("written");
    assert_eq!(rewritten.text, written.text, "the same identity");
    // The resumed run loses the torch's transform after the file is written, and is carried into
    // it cooked at last: once, without the transform.
    let torch = all_named(&resumed, "torch")[0];
    resumed.remove::<Transform>(torch);
    let mut cooked = torch_scenes();
    cooked.insert_file(TORCH_ZONE, "zone", written.text);
    let (carried, report) = resumed
        .carry(
            &zone_start(&mut scenes),
            zone_start(&mut cooked),
            &mut cooked,
        )
        .expect("carried");
    assert!(report.set_aside.is_empty(), "{report:?}");
    let torches = all_named(&carried, "torch");
    assert_eq!(torches.len(), 1, "the torch exists once");
    assert!(
        carried.get::<Transform>(torches[0]).is_none(),
        "as the run holds it"
    );
}
