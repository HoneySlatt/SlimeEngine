//! Scene files placed in worlds: prefabs and their overrides, nesting, references, the changes of a
//! prefab followed, and files that say something wrong (`docs/adr/0089-scenes-and-prefabs.md`).

use glam::{Mat4, Vec3};
use slime_assets::{AssetId, CookedScene, SceneNode};
use slime_reflect::{Reflect, Value};
use slime_world::{
    Component, Entity, Name, PersistentId, SceneError, SceneLibrary, World, WorldError,
};

/// A colour.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Tint")]
struct Tint {
    r: f32,
    g: f32,
    b: f32,
}

/// How a lamp glows.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Glow")]
struct Glow {
    #[reflect(range(min = 0.0))]
    intensity: f32,
    tint: Tint,
}
impl Component for Glow {}

/// What an entity looks at.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Aim")]
struct Aim {
    at: Entity,
}
impl Component for Aim {}

const YARD: AssetId = AssetId::from_u128(1);
const LAMP: AssetId = AssetId::from_u128(2);
const VILLAGE: AssetId = AssetId::from_u128(3);
const CHAPEL: AssetId = AssetId::from_u128(9);

/// A lamp on a post: its light aims at the post, and its glass is hidden.
const LAMP_FILE: &str = r#"
[entities.post.components]
Name = "post"
Transform = { translation = { x = 0.0, y = 1.0, z = 0.0 } }

[entities.light]
parent = "post"

[entities.light.components]
"test::Glow" = { intensity = 40.0, tint = { r = 1.0, g = 0.8, b = 0.6 } }
"test::Aim" = { at = "post" }

[entities.glass]
parent = "post"

[entities.glass.components]
Hidden = {}
"#;

/// The lamp placed three times: the gate's moved and dimmer, the shed's light hidden, the porch's
/// glass removed.
const YARD_FILE: &str = r#"
[assets]
lamp = { id = "00000000-0000-0000-0000-000000000002", path = "lamp.scene" }

[entities.porch]
prefab = "lamp"

[entities.gate]
prefab = "lamp"

[entities.shed]
prefab = "lamp"

[changed.gate.Transform]
translation = { x = 4.0 }

[changed."gate/light"."test::Glow"]
intensity = 10.0

[added."shed/light"]
Hidden = {}

[removed]
entities = ["porch/glass"]
"#;

/// A world that knows the tests' components.
fn world() -> World {
    let mut world = World::new();
    world.register::<Glow>();
    world.register::<Aim>();
    world
}

/// A library of scene files: their IDs, names and texts.
fn library(files: &[(AssetId, &str, &str)]) -> SceneLibrary {
    let mut library = SceneLibrary::new();
    for &(id, name, text) in files {
        library.insert_file(id, name, text);
    }
    library
}

/// Spawns the scene `id` of `library` in `world` under `key`.
fn spawn(world: &mut World, library: &mut SceneLibrary, id: AssetId, key: &str) -> Entity {
    let scene = library.resolve(world.registry(), id).expect("resolved");
    world.spawn_scene(key, &scene).expect("spawned")
}

/// The entity whose persistent identity `id` writes.
fn entity(world: &World, id: &str) -> Entity {
    world
        .find(&id.parse().expect("an identity"))
        .unwrap_or_else(|| panic!("no `{id}`"))
}

/// The value of the component `component` of the entity `id`, if it holds one.
fn value(world: &World, id: &str, component: &str) -> Option<Value> {
    world.component_value(entity(world, id), component)
}

/// The persistent identity of the entity's parent.
fn parent(world: &World, id: &str) -> Option<String> {
    world
        .parent(entity(world, id))
        .and_then(|parent| world.persistent_id(parent))
        .map(|id| id.to_string())
}

#[test]
fn a_prefab_placed_three_times_gives_three_subtrees_equal_to_it_but_where_overridden() {
    let mut scenes = library(&[(YARD, "yard", YARD_FILE), (LAMP, "lamp", LAMP_FILE)]);
    let mut lamp = world();
    spawn(&mut lamp, &mut scenes, LAMP, "lamp");
    let mut yard = world();
    let root = spawn(&mut yard, &mut scenes, YARD, "yard");
    let children: Vec<String> = yard
        .children(root)
        .iter()
        .map(|&child| yard.persistent_id(child).expect("named").to_string())
        .collect();
    assert_eq!(children, ["yard/porch", "yard/gate", "yard/shed"]);
    for placement in ["porch", "gate", "shed"] {
        assert_eq!(
            parent(&yard, &format!("yard/{placement}/light")),
            Some(format!("yard/{placement}/post"))
        );
        assert_eq!(
            value(&yard, &format!("yard/{placement}"), "Name"),
            Some(Name(placement.to_owned()).to_value()),
            "a placement's root is named after its ID"
        );
        for (id, component) in [
            ("post", "Transform"),
            ("post", "Name"),
            ("light", "test::Glow"),
        ] {
            let overridden = placement == "gate" && component == "test::Glow";
            let ours = value(&yard, &format!("yard/{placement}/{id}"), component);
            let prefab = value(&lamp, &format!("lamp/{id}"), component);
            assert_eq!(ours != prefab, overridden, "{placement}/{id}: {component}");
        }
    }
    let gate = Glow::from_value(&value(&yard, "yard/gate/light", "test::Glow").expect("held"));
    assert_eq!(
        gate.map(|glow| (glow.intensity, glow.tint.g)),
        Ok((10.0, 0.8)),
        "the intensity overridden, the tint the prefab's"
    );
    let moved = yard
        .global_transform(entity(&yard, "yard/gate/post"))
        .expect("placed")
        .translation;
    assert!((moved - glam::DVec3::new(4.0, 1.0, 0.0)).length() < 1e-9);
    assert!(value(&yard, "yard/shed/light", "Hidden").is_some());
    assert!(value(&yard, "yard/gate/light", "Hidden").is_none());
    assert!(
        yard.find(&"yard/porch/glass".parse().unwrap()).is_none(),
        "removed"
    );
    assert!(yard.find(&"yard/gate/glass".parse().unwrap()).is_some());
    let placement = yard
        .placement(entity(&yard, "yard/gate"))
        .expect("a placement");
    assert_eq!(placement.prefab().name(), "lamp");
    assert_eq!(placement.prefab().id(), LAMP);
    assert_eq!(placement.overrides(), 2);
    assert!(placement.set_aside().is_empty());
    assert!(yard.placement(entity(&yard, "yard/gate/light")).is_none());
}

#[test]
fn placements_follow_their_prefab_where_no_override_changes_it() {
    // The shed sets the intensity that the prefab later takes, and keeps it when the prefab changes
    // again; the gate overrides the glass's hiding, and the light, both of which the prefab loses.
    let yard = format!(
        "{YARD_FILE}\n[changed.\"shed/light\".\"test::Glow\"]\nintensity = 50.0\n\n\
         [removed.components]\n\"gate/glass\" = [\"Hidden\"]\n\n\
         [added.\"gate/light\".\"test::Aim\"]\nat = \"gate/post\"\n"
    );
    let changed = LAMP_FILE
        .replace("intensity = 40.0", "intensity = 50.0")
        .replace(
            "[entities.glass]\nparent = \"post\"\n\n[entities.glass.components]\nHidden = {}\n",
            "[entities.hook]\nparent = \"post\"\n",
        )
        .replace("[entities.light]", "[entities.lantern]")
        .replace(
            "[entities.light.components]",
            "[entities.lantern.components]",
        );
    let mut scenes = library(&[(YARD, "yard", &yard), (LAMP, "lamp", LAMP_FILE)]);
    let mut before = world();
    spawn(&mut before, &mut scenes, YARD, "yard");
    let glow = |world: &World, id: &str| {
        Glow::from_value(&value(world, id, "test::Glow").expect("held"))
            .expect("a glow")
            .intensity
    };
    assert_eq!(glow(&before, "yard/porch/light"), 40.0);
    assert_eq!(glow(&before, "yard/shed/light"), 50.0);
    scenes.insert_file(LAMP, "lamp", changed.clone());
    let resolved = scenes.resolve(before.registry(), YARD).expect("resolved");
    let mut after = world();
    after.spawn_scene("yard", &resolved).expect("spawned");
    for placement in ["porch", "gate", "shed"] {
        assert!(
            after
                .find(&format!("yard/{placement}/hook").parse().unwrap())
                .is_some(),
            "a new entity appears in every placement"
        );
        assert_eq!(glow(&after, &format!("yard/{placement}/lantern")), 50.0);
    }
    // The lamp's light is the lantern now: what the yard wrote for the light, and for the glass it no
    // longer has, is set aside, and kept.
    let aside: Vec<(String, Option<String>)> = resolved
        .set_aside()
        .iter()
        .map(|aside| (aside.entity.clone(), aside.component.clone()))
        .collect();
    assert_eq!(
        aside,
        [
            ("porch/glass".to_owned(), None),
            ("gate/glass".to_owned(), Some("Hidden".to_owned())),
            ("gate/light".to_owned(), Some("test::Aim".to_owned())),
            ("gate/light".to_owned(), Some("test::Glow".to_owned())),
            ("shed/light".to_owned(), Some("Hidden".to_owned())),
            ("shed/light".to_owned(), Some("test::Glow".to_owned())),
        ]
    );
    let gate = after
        .placement(entity(&after, "yard/gate"))
        .expect("a placement");
    assert_eq!(gate.set_aside().len(), 3);
    assert_eq!(gate.overrides(), 4, "set aside, and kept");
    // The prefab changes again: the shed's light, written in a file that sets 50 again, keeps 50.
    let lantern = changed.replace("intensity = 50.0", "intensity = 70.0");
    let yard_lantern = yard.replace(
        "\"shed/light\".\"test::Glow\"",
        "\"shed/lantern\".\"test::Glow\"",
    );
    let mut scenes = library(&[(YARD, "yard", &yard_lantern), (LAMP, "lamp", &lantern)]);
    let mut again = world();
    spawn(&mut again, &mut scenes, YARD, "yard");
    assert_eq!(glow(&again, "yard/porch/lantern"), 70.0);
    assert_eq!(
        glow(&again, "yard/shed/lantern"),
        50.0,
        "an override equal to its prefab's stays"
    );
}

#[test]
fn nested_placements_apply_the_innermost_scene_first_and_the_outermost_wins() {
    let village = r#"
[assets]
lamp = { id = "00000000-0000-0000-0000-000000000002" }

[entities.square]
prefab = "lamp"

[changed."square/light"."test::Glow"]
intensity = 20.0
tint = { r = 0.5 }
"#;
    let yard = r#"
[assets]
village = { id = "00000000-0000-0000-0000-000000000003" }

[entities.north]
prefab = "village"

[entities.south]
prefab = "village"

[changed."north/square/light"."test::Glow"]
intensity = 30.0
"#;
    let mut scenes = library(&[
        (YARD, "yard", yard),
        (VILLAGE, "village", village),
        (LAMP, "lamp", LAMP_FILE),
    ]);
    let mut world = world();
    spawn(&mut world, &mut scenes, YARD, "yard");
    let glow =
        |id: &str| Glow::from_value(&value(&world, id, "test::Glow").expect("held")).unwrap();
    assert_eq!(
        glow("yard/north/square/light").intensity,
        30.0,
        "the outermost wins"
    );
    assert_eq!(
        glow("yard/north/square/light").tint.r,
        0.5,
        "the inner one stays where the outer one says nothing"
    );
    assert_eq!(glow("yard/south/square/light").intensity, 20.0);
    assert_eq!(
        parent(&world, "yard/north/square"),
        Some("yard/north".to_owned())
    );
    assert!(
        world
            .placement(entity(&world, "yard/north/square"))
            .is_some()
    );
    assert_eq!(
        world
            .placement(entity(&world, "yard/north"))
            .expect("a placement")
            .prefab()
            .name(),
        "village"
    );
}

#[test]
fn prefabs_that_place_each_other_or_nest_too_deep_are_refused() {
    let places = |id: u128| {
        format!(
            "[assets]\nnext = {{ id = \"{}\" }}\n\n[entities.inner]\nprefab = \"next\"\n",
            AssetId::from_u128(id)
        )
    };
    let mut scenes = library(&[(YARD, "a", &places(3)), (VILLAGE, "b", &places(1))]);
    let world = world();
    match scenes.resolve(world.registry(), YARD) {
        Err(SceneError::Cycle(chain)) => assert_eq!(chain, ["a", "b", "a"]),
        other => panic!("{other:?}"),
    }
    // Seventeen scenes, each placing the next.
    let mut scenes = SceneLibrary::new();
    for id in 100..116 {
        scenes.insert_file(AssetId::from_u128(id), format!("s{id}"), places(id + 1));
    }
    scenes.insert_file(AssetId::from_u128(116), "s116", "");
    match scenes.resolve(world.registry(), AssetId::from_u128(100)) {
        Err(SceneError::TooDeep(chain)) => assert_eq!(chain.len(), 17),
        other => panic!("{other:?}"),
    }
    scenes
        .resolve(world.registry(), AssetId::from_u128(101))
        .expect("sixteen scenes nest");
    let mut scenes = library(&[(YARD, "yard", YARD_FILE)]);
    let error = scenes.resolve(world.registry(), YARD).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!("scene `yard` places the scene {LAMP}, which the library does not hold")
    );
}

#[test]
fn a_gltf_scene_placed_as_a_prefab_gives_what_spawning_it_alone_gives() {
    let node = |name: &str, parent: Option<u32>, at: Vec3| SceneNode {
        name: name.to_owned(),
        parent,
        transform: Mat4::from_translation(at).to_cols_array(),
        meshes: vec![AssetId::from_u128(7)],
        light: None,
        extras: Vec::new(),
    };
    let chapel = CookedScene {
        name: "chapel".to_owned(),
        nodes: vec![
            node("nave", None, Vec3::new(0.0, 0.0, -30.0)),
            node("door", Some(0), Vec3::new(0.0, 0.0, 10.5)),
        ],
    };
    let yard = format!(
        "[assets]\nchapel = {{ id = \"{CHAPEL}\" }}\n\n[entities.chapel]\nprefab = \"chapel\"\n\n\
         [changed.chapel.Transform]\ntranslation = {{ x = 5.0 }}\n"
    );
    let mut scenes = library(&[(YARD, "yard", &yard)]);
    scenes.insert_cooked(CHAPEL, chapel);
    let mut alone = world();
    spawn(&mut alone, &mut scenes, CHAPEL, "zone");
    let mut placed = world();
    spawn(&mut placed, &mut scenes, YARD, "yard");
    for node in ["nave", "door"] {
        for component in ["Transform", "Name", "Meshes"] {
            assert_eq!(
                value(&alone, &format!("zone/{node}"), component),
                value(&placed, &format!("yard/chapel/{node}"), component),
                "{node}: {component}"
            );
        }
    }
    assert_eq!(
        parent(&placed, "yard/chapel/door"),
        Some("yard/chapel/nave".to_owned())
    );
    let door = placed
        .global_transform(entity(&placed, "yard/chapel/door"))
        .expect("placed")
        .translation;
    assert!((door - glam::DVec3::new(5.0, 0.0, -19.5)).length() < 1e-9);
}

#[test]
fn references_name_each_placements_own_entities_and_paths_from_the_files_root() {
    let yard = format!(
        "{YARD_FILE}\n[entities.sign.components]\n\"test::Aim\" = {{ at = \"gate/light\" }}\n\n\
         [entities.post.components]\n\"test::Aim\" = {{ at = \".\" }}\n\n\
         [entities.stone.components]\n\"test::Aim\" = {{ at = \"\" }}\n"
    );
    let mut scenes = library(&[(YARD, "yard", &yard), (LAMP, "lamp", LAMP_FILE)]);
    let mut world = world();
    let root = spawn(&mut world, &mut scenes, YARD, "yard");
    let aim = |id: &str| {
        Aim::from_value(&value(&world, id, "test::Aim").expect("held"))
            .expect("an aim")
            .at
    };
    for placement in ["porch", "gate", "shed"] {
        assert_eq!(
            aim(&format!("yard/{placement}/light")),
            entity(&world, &format!("yard/{placement}/post"))
        );
    }
    assert_eq!(aim("yard/sign"), entity(&world, "yard/gate/light"));
    assert_eq!(aim("yard/post"), root);
    assert!(!world.contains(aim("yard/stone")), "no entity");
}

#[test]
fn a_file_that_says_something_wrong_refuses_its_scene_and_names_what() {
    let lamp_with = |component: &str| {
        LAMP_FILE.replace(
            "\"test::Aim\" = { at = \"post\" }",
            &format!("\"test::Aim\" = {{ at = \"post\" }}\n{component}"),
        )
    };
    let cases = [
        (lamp_with("\"test::Nothing\" = {}"), "test::Nothing"),
        (
            LAMP_FILE.replace("intensity = 40.0,", "intensity = 40.0, flicker = 2.0,"),
            "no field `flicker`",
        ),
        (LAMP_FILE.replace("40.0", "-1.0"), "is not ≥ 0"),
        (lamp_with("Hidden = 3"), "Hidden"),
        (
            LAMP_FILE.replace("at = \"post\"", "at = \"shed\""),
            "no entity is named `shed`",
        ),
        (
            LAMP_FILE.replace(
                "parent = \"post\"\n\n[entities.light.components]",
                "parent = \"gate\"\n\n[entities.light.components]",
            ),
            "no entity `gate` in this scene",
        ),
        (
            format!("{LAMP_FILE}\n[entities.post]\nparent = \"light\"\n"),
            "own ancestor",
        ),
        (format!("layout = 2\n{LAMP_FILE}"), "layout 2"),
        (format!("lights = 3\n{LAMP_FILE}"), "no field `lights`"),
        (format!("{LAMP_FILE}\n[entities.\"a/b\"]\n"), "holds `/`"),
    ];
    let mut world = world();
    for (text, says) in cases {
        let mut scenes = library(&[(LAMP, "lamp", &text)]);
        match scenes.resolve(world.registry(), LAMP) {
            Err(SceneError::File { scene, error }) => {
                assert_eq!(scene, "lamp");
                assert!(
                    error.to_string().contains(says),
                    "{error} does not say {says:?}"
                );
                assert!(error.line_and_column().is_some(), "{error}");
            }
            other => panic!("{says}: {other:?}"),
        }
    }
    let yard_with = |more: &str| format!("{YARD_FILE}\n{more}");
    let cases = [
        (yard_with("[changed.post.Name]\n"), "no placement `post`"),
        (
            yard_with("[entities.post.components]\nName = \"post\"\n\n[changed.post.Name]\n"),
            "not a placement",
        ),
        (
            YARD_FILE.replace(
                "[entities.shed]\nprefab = \"lamp\"",
                "[entities.shed]\nprefab = \"lamp\"\ncomponents = {}",
            ),
            "a placement's components are its prefab's",
        ),
        (
            YARD_FILE.replace(
                "prefab = \"lamp\"\n\n[entities.gate]",
                "prefab = \"lamps\"\n\n[entities.gate]",
            ),
            "no asset `lamps`",
        ),
        (
            YARD_FILE.replace("entities = [\"porch/glass\"]", "entities = [\"porch\"]"),
            "remove its record",
        ),
        (
            YARD_FILE.replace("00000000-0000-0000-0000-000000000002", "two"),
            "not an asset ID",
        ),
    ];
    for (text, says) in cases {
        let mut scenes = library(&[(YARD, "yard", &text), (LAMP, "lamp", LAMP_FILE)]);
        match scenes.resolve(world.registry(), YARD) {
            Err(SceneError::File { error, .. }) => {
                assert!(
                    error.to_string().contains(says),
                    "{error} does not say {says:?}"
                );
            }
            other => panic!("{says}: {other:?}"),
        }
    }
    // A rule of a type's own check, and a type that the world stores no component of, are found as
    // the scene spawns, which then spawns nothing.
    let unturned = LAMP_FILE.replace(
        "translation = { x = 0.0, y = 1.0, z = 0.0 }",
        "translation = { x = 0.0, y = 1.0, z = 0.0 }, rotation = { x = 0.0, y = 0.0, z = 0.0, w = 2.0 }",
    );
    let mut scenes = library(&[(LAMP, "lamp", &unturned)]);
    let scene = scenes
        .resolve(world.registry(), LAMP)
        .expect("its kinds and ranges are right");
    match world.spawn_scene("lamp", &scene) {
        Err(WorldError::RefusedInScene {
            entity, component, ..
        }) => {
            assert_eq!(entity, PersistentId::new(["lamp", "post"]));
            assert_eq!(component, "Transform");
        }
        other => panic!("{other:?}"),
    }
    assert!(world.is_empty());
    let mut scenes = library(&[(LAMP, "lamp", &lamp_with("String = \"a text\""))]);
    let scene = scenes
        .resolve(world.registry(), LAMP)
        .expect("a registered type");
    assert_eq!(
        world.spawn_scene("lamp", &scene),
        Err(WorldError::UnknownComponent("String".to_owned()))
    );
    assert!(world.is_empty());
    assert!(world.find(&PersistentId::new(["lamp", "post"])).is_none());
}

#[test]
fn scene_files_cut_or_changed_at_every_character_are_refused_or_spawned_without_a_panic() {
    let mut scenes = library(&[(LAMP, "lamp", LAMP_FILE)]);
    let registry = world();
    let characters: Vec<(usize, char)> = YARD_FILE.char_indices().collect();
    for &(at, _) in &characters {
        for text in [
            YARD_FILE[..at].to_owned(),
            format!("{}\"{}", &YARD_FILE[..at], &YARD_FILE[at..]),
            format!("{}]{}", &YARD_FILE[..at], &YARD_FILE[at..]),
        ] {
            scenes.insert_file(YARD, "yard", text);
            if let Ok(scene) = scenes.resolve(registry.registry(), YARD) {
                let mut world = world();
                let _ = world.spawn_scene("yard", &scene);
            }
        }
    }
}

/// A prefab of `entities` entities, each a child of the one before, with a name, a transform and
/// a glow.
fn chain_prefab(entities: usize) -> String {
    let mut text = String::new();
    for index in 0..entities {
        text.push_str(&format!("[entities.e{index}]\n"));
        if index > 0 {
            text.push_str(&format!("parent = \"e{}\"\n", index - 1));
        }
        text.push_str(&format!(
            "\n[entities.e{index}.components]\nName = \"e{index}\"\n\
             Transform = {{ translation = {{ x = 0.0, y = 1.0, z = 0.0 }} }}\n\
             \"test::Glow\" = {{ intensity = 1.0, tint = {{ r = 1.0, g = 1.0, b = 1.0 }} }}\n\n"
        ));
    }
    text
}

/// A scene that places the lamp `placements` times, each moved, and overrides a value of each.
fn many_placements(placements: usize) -> String {
    let mut text = format!("[assets]\nlamp = {{ id = \"{LAMP}\" }}\n\n");
    for index in 0..placements {
        text.push_str(&format!("[entities.p{index}]\nprefab = \"lamp\"\n\n"));
    }
    for index in 0..placements {
        text.push_str(&format!(
            "[changed.p{index}.Transform]\ntranslation = {{ x = {index}.0 }}\n\n\
             [changed.\"p{index}/e0\".\"test::Glow\"]\nintensity = 2.0\n\n"
        ));
    }
    text
}

#[test]
#[ignore = "a measurement, run in a release build: cargo test --release -p slime-world -- --ignored --nocapture"]
fn what_resolving_and_spawning_placements_costs() {
    for (placements, entities) in [(1_000, 10), (1_000, 100)] {
        let mut scenes = library(&[
            (YARD, "yard", &many_placements(placements)),
            (LAMP, "lamp", &chain_prefab(entities)),
        ]);
        let mut world = world();
        let started = std::time::Instant::now();
        let scene = scenes.resolve(world.registry(), YARD).expect("resolved");
        let resolved = started.elapsed();
        let started = std::time::Instant::now();
        world.spawn_scene("yard", &scene).expect("spawned");
        let spawned = started.elapsed();
        eprintln!(
            "{placements} placements of {entities} entities: {} entities, resolved in {resolved:?}, spawned in {spawned:?}",
            world.len()
        );
        assert_eq!(world.len(), placements * (entities + 1) + 1);
    }
}
