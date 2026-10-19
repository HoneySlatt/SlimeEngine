//! Saves loaded into a world made again: what they name, and what they hold that a world cannot
//! take (`docs/adr/0087-the-worlds-documents.md`,
//! `docs/adr/0093-saves-and-hot-reload-with-placements.md`).

use glam::Mat4;
use slime_assets::{AssetId, CookedScene, SceneNode};
use slime_reflect::Reflect;
use slime_world::{Component, DocumentError, Entity, ResolvedScene, SceneLibrary, World};

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
