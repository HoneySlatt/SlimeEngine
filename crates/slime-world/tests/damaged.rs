//! A world's files damaged (`docs/adr/0096-the-files-the-engine-reads.md`): a save and a scene
//! file, cut at every byte, their bytes changed and mutated at random, are refused or loaded, and
//! never make the world panic.

#[path = "../../slime-assets/tests/common/damage.rs"]
mod damage;

use std::collections::BTreeMap;

use damage::damage;
use glam::{DVec3, Mat4};
use slime_assets::{AssetId, CookedScene, SceneNode};
use slime_reflect::Reflect;
use slime_world::{
    Component, Entity, Hidden, Name, ResolvedScene, SceneLibrary, Transform, Unsaved, World,
};

/// What an entity aims at.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Aim")]
struct Aim {
    at: Entity,
}
impl Component for Aim {}

/// A glTF scene of a gate and a well, resolved.
fn yard() -> ResolvedScene {
    let node = |name: &str| SceneNode {
        name: name.to_owned(),
        parent: None,
        transform: Mat4::IDENTITY.to_cols_array(),
        meshes: Vec::new(),
        light: None,
        extras: Vec::new(),
        components: Vec::new(),
    };
    let scene = CookedScene {
        name: "yard".to_owned(),
        nodes: vec![node("gate"), node("well")],
    };
    ResolvedScene::from_cooked(AssetId::from_u128(1), &scene, World::new().registry())
        .expect("resolved")
}

/// The world that a run starts from: the yard, a hero created under a key, and a stone created
/// without one.
fn start() -> World {
    let mut world = World::new();
    world.register::<Aim>();
    world.spawn_scene("yard", &yard()).expect("spawned");
    let hero = world.spawn_keyed("hero").expect("a key");
    world.insert(hero, Name("hero".to_owned()));
    let stone = world.spawn();
    world.insert(stone, Transform::IDENTITY);
    world
}

#[test]
fn a_damaged_save_is_refused_or_loaded_never_a_panic() {
    // A run that changed, added, removed and created something of every kind that a save says.
    let mut run = start();
    let find = |world: &World, id: &str| {
        world
            .find(&id.parse().expect("an identity"))
            .expect("an entity")
    };
    let (gate, well, hero) = (
        find(&run, "yard/gate"),
        find(&run, "yard/well"),
        find(&run, "hero"),
    );
    run.insert(gate, Transform::from_translation(DVec3::new(1.0, 2.0, 3.0)));
    run.insert(gate, Aim { at: hero });
    run.insert(well, Hidden);
    run.set_parent(hero, Some(gate)).expect("a parent");
    let arrow = run.spawn();
    run.insert(arrow, Aim { at: well });
    let spark = run.spawn();
    run.insert(spark, Unsaved);
    run.despawn(find(&run, "created/1"));
    let save = run
        .save(
            &start(),
            &BTreeMap::from([("game".to_owned(), "0.5".to_owned())]),
        )
        .expect("saved");
    assert!(start().load(&save, &mut SceneLibrary::new()).is_ok());
    damage(save.as_bytes(), |bytes| {
        let mut world = start();
        let _ = world.load(&String::from_utf8_lossy(bytes), &mut SceneLibrary::new());
    });
}

#[test]
fn a_damaged_scene_file_is_refused_or_spawned_never_a_panic() {
    let lamp = "[entities.post.components]\nName = \"post\"\n\
                Transform = { translation = { x = 0.0, y = 1.0, z = 0.0 } }\n\n\
                [entities.light]\nparent = \"post\"\n\n[entities.light.components]\n\
                Light = { Point = { color = { r = 1.0, g = 0.8, b = 0.6, a = 1.0 }, intensity = 40.0 } }\n\
                \"test::Aim\" = { at = \"post\" }\n";
    let yard = format!(
        "[assets]\nlamp = {{ id = \"{}\" }}\n\n[root.components]\nName = \"yard\"\n\n\
         [entities.gate]\nprefab = \"lamp\"\n\n[entities.porch]\nprefab = \"lamp\"\n\n\
         [entities.sign]\nparent = \"gate/post\"\n\n[entities.sign.components]\n\
         \"test::Aim\" = {{ at = \"porch/light\" }}\n\n\
         [changed.\"gate/light\".Light.Point]\nintensity = 10.0\n\n\
         [removed]\nentities = [\"porch/light\"]\n",
        AssetId::from_u128(2)
    );
    let mut scenes = SceneLibrary::new();
    scenes.insert_file(AssetId::from_u128(2), "lamp", lamp);
    let registry = start();
    scenes.insert_file(AssetId::from_u128(3), "yard", yard.clone());
    let resolved = scenes
        .resolve(registry.registry(), AssetId::from_u128(3))
        .expect("resolved");
    assert!(
        resolved.set_aside().is_empty(),
        "{:?}",
        resolved.set_aside()
    );
    damage(yard.as_bytes(), |bytes| {
        scenes.insert_file(
            AssetId::from_u128(3),
            "yard",
            String::from_utf8_lossy(bytes).into_owned(),
        );
        if let Ok(scene) = scenes.resolve(registry.registry(), AssetId::from_u128(3)) {
            let mut world = start();
            let _ = world.spawn_scene("placed", &scene);
        }
    });
}
