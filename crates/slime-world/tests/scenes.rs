//! Scene files placed in worlds: prefabs and their overrides, nesting, references, the changes of a
//! prefab followed, and files that say something wrong (`docs/adr/0089-scenes-and-prefabs.md`).

use glam::{Mat4, Vec3};
use slime_assets::{AssetId, CookedScene, SceneNode};
use slime_reflect::{Reflect, Value};
use slime_world::{
    Component, Entity, Hidden, Name, PersistentId, SceneError, SceneLibrary, Transform, World,
    WorldError,
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
#[reflect(name = "test::Glow", alias = "test::Shine")]
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
    assert_eq!(placement.override_count(), 2);
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
    assert_eq!(gate.override_count(), 4, "set aside, and kept");
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
    let mut door = node("door", Some(0), Vec3::new(0.0, 0.0, 10.5));
    door.light = Some(slime_assets::SceneLight {
        kind: slime_assets::LightKind::Point,
        color: [1.0, 0.9, 0.7],
        intensity: 12.0,
        range: Some(6.0),
    });
    door.extras = vec![(
        "marker".to_owned(),
        slime_assets::Extra::Text("door".to_owned()),
    )];
    let chapel = CookedScene {
        name: "chapel".to_owned(),
        nodes: vec![node("nave", None, Vec3::new(0.0, 0.0, -30.0)), door],
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
    let under = |world: &World, key: &str| {
        let mut ids: Vec<String> = world
            .entities()
            .filter_map(|entity| world.persistent_id(entity))
            .map(|id| id.to_string())
            .filter_map(|id| id.strip_prefix(key).map(str::to_owned))
            .collect();
        ids.sort();
        ids
    };
    assert_eq!(under(&alone, "zone/"), under(&placed, "yard/chapel/"));
    for node in ["nave", "door"] {
        for component in ["Transform", "Name", "Meshes", "Light", "Extras"] {
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
    // Every section: the root's components, an entity of the file under a placement, references,
    // and every kind of override.
    let yard = format!(
        "{YARD_FILE}\n[root.components]\nName = \"yard\"\n\n[entities.sign]\nparent = \"gate/post\"\n\n\
         [entities.sign.components]\n\"test::Aim\" = {{ at = \"porch/light\" }}\n\n\
         [removed.components]\n\"gate/glass\" = [\"Hidden\"]\n"
    );
    let characters: Vec<(usize, char)> = yard.char_indices().collect();
    for &(at, _) in &characters {
        for text in [
            yard[..at].to_owned(),
            format!("{}\"{}", &yard[..at], &yard[at..]),
            format!("{}]{}", &yard[..at], &yard[at..]),
        ] {
            scenes.insert_file(YARD, "yard", text);
            if let Ok(scene) = scenes.resolve(registry.registry(), YARD) {
                let mut world = world();
                let _ = world.spawn_scene("yard", &scene);
            }
        }
    }
}

/// The aims and the set-aside overrides of a resolved scene, as pairs of the entity and the
/// component, for comparisons.
fn aside_of(scene: &slime_world::ResolvedScene) -> Vec<(String, Option<String>)> {
    scene
        .set_aside()
        .iter()
        .map(|aside| (aside.entity.clone(), aside.component.clone()))
        .collect()
}

#[test]
fn what_the_file_adds_under_and_refers_to_in_a_prefab_entity_gone_is_kept_and_reported() {
    let yard = format!(
        "{YARD_FILE}\n[entities.sign]\nparent = \"gate/glass\"\n\n\
         [entities.sign.components]\n\"test::Aim\" = {{ at = \"gate/glass\" }}\n\n\
         [added.\"gate/light\".\"test::Aim\"]\nat = \"gate/glass\"\n"
    );
    let pane = LAMP_FILE
        .replace("[entities.glass]", "[entities.pane]")
        .replace("[entities.glass.components]", "[entities.pane.components]");
    let mut scenes = library(&[(YARD, "yard", &yard), (LAMP, "lamp", &pane)]);
    let mut world = world();
    let root = spawn(&mut world, &mut scenes, YARD, "yard");
    let scene = scenes.resolve(world.registry(), YARD).expect("resolved");
    assert_eq!(
        aside_of(&scene),
        [
            ("porch/glass".to_owned(), None),
            ("sign".to_owned(), None),
            ("sign".to_owned(), Some("test::Aim".to_owned())),
            ("gate/light".to_owned(), Some("test::Aim".to_owned())),
        ]
    );
    assert_eq!(world.parent(entity(&world, "yard/sign")), Some(root));
    for id in ["yard/sign", "yard/gate/light"] {
        let aim = Aim::from_value(&value(&world, id, "test::Aim").expect("held")).expect("an aim");
        assert!(!world.contains(aim.at), "{id} aims at no entity");
    }
    // Without the prefab entity, a path into a placement still names a placement of the file.
    let typo = yard.replace("parent = \"gate/glass\"", "parent = \"gates/glass\"");
    let mut scenes = library(&[(YARD, "yard", &typo), (LAMP, "lamp", &pane)]);
    let error = scenes.resolve(world.registry(), YARD).unwrap_err();
    assert!(
        error.to_string().contains("no entity `gates/glass`"),
        "{error}"
    );
}

#[test]
fn what_the_file_makes_under_an_entity_that_it_removes_stays_and_is_reported() {
    let yard = |removed: &str| {
        format!(
            "[assets]\nlamp = {{ id = \"{LAMP}\" }}\n\n[entities.gate]\nprefab = \"lamp\"\n\n\
             [entities.sign]\nparent = \"gate/glass\"\n\n[entities.porch]\nprefab = \"lamp\"\n\
             parent = \"gate/glass\"\n\n[entities.post.components]\n\"test::Aim\" = {{ at = \"gate/light\" }}\n\n\
             [removed]\nentities = [{removed}]\n"
        )
    };
    let mut reports = Vec::new();
    for removed in [
        "\"gate/glass\", \"gate/post\"",
        "\"gate/post\", \"gate/glass\"",
    ] {
        let text = yard(removed);
        let mut scenes = library(&[(YARD, "yard", &text), (LAMP, "lamp", LAMP_FILE)]);
        let mut world = world();
        let root = spawn(&mut world, &mut scenes, YARD, "yard");
        for id in ["yard/sign", "yard/porch"] {
            assert_eq!(world.parent(entity(&world, id)), Some(root), "{id} stays");
        }
        assert!(world.find(&"yard/porch/light".parse().unwrap()).is_some());
        assert!(world.placement(entity(&world, "yard/porch")).is_some());
        for gone in ["yard/gate/post", "yard/gate/light", "yard/gate/glass"] {
            assert!(world.find(&gone.parse().unwrap()).is_none(), "{gone}");
        }
        let aim = Aim::from_value(&value(&world, "yard/post", "test::Aim").expect("held"));
        assert!(
            !world.contains(aim.expect("an aim").at),
            "a reference to an entity removed"
        );
        let scene = scenes.resolve(world.registry(), YARD).expect("resolved");
        reports.push(aside_of(&scene));
    }
    assert_eq!(
        reports[0],
        [("sign".to_owned(), None), ("porch".to_owned(), None)]
    );
    assert_eq!(reports[0], reports[1], "whatever the order of the list");
}

#[test]
fn a_component_given_twice_for_one_entity_by_its_name_and_an_alias_is_refused() {
    let world = world();
    for more in [
        "[changed.\"gate/light\".\"test::Glow\"]\nintensity = 1.0\n\n\
         [changed.\"gate/light\".\"test::Shine\"]\nintensity = 2.0\n",
        "[added.\"gate/post\".\"test::Glow\"]\nintensity = 1.0\ntint = { r = 1.0, g = 1.0, b = 1.0 }\n\n\
         [added.\"gate/post\".\"test::Shine\"]\nintensity = 1.0\ntint = { r = 1.0, g = 1.0, b = 1.0 }\n",
        "[removed.components]\n\"gate/glass\" = [\"Hidden\", \"Hidden\"]\n",
    ] {
        let text = YARD_FILE
            .replace(
                "[changed.\"gate/light\".\"test::Glow\"]\nintensity = 10.0\n",
                "",
            )
            .replace("[removed]\nentities = [\"porch/glass\"]\n", "")
            + more;
        let mut scenes = library(&[(YARD, "yard", &text), (LAMP, "lamp", LAMP_FILE)]);
        match scenes.resolve(world.registry(), YARD) {
            Err(SceneError::File { error, .. }) => {
                assert!(error.to_string().contains("is given twice"), "{error}");
            }
            other => panic!("{more}: {other:?}"),
        }
    }
}

#[test]
fn a_scene_of_more_entities_than_the_limit_is_refused_before_any_is_made() {
    let mut scenes = library(&[
        (YARD, "yard", &many_placements(1_000)),
        (LAMP, "lamp", &chain_prefab(1_100)),
    ]);
    let world = world();
    match scenes.resolve(world.registry(), YARD) {
        Err(SceneError::TooLarge { scene }) => assert_eq!(scene, "yard"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_gltf_scene_counts_in_the_depth_of_the_placements_that_nest_it() {
    let places = |id: u128| {
        format!(
            "[assets]\nnext = {{ id = \"{}\" }}\n\n[entities.inner]\nprefab = \"next\"\n",
            AssetId::from_u128(id)
        )
    };
    let mut scenes = SceneLibrary::new();
    for id in 100..116 {
        scenes.insert_file(AssetId::from_u128(id), format!("s{id}"), places(id + 1));
    }
    scenes.insert_cooked(
        AssetId::from_u128(116),
        CookedScene {
            name: "s116".to_owned(),
            nodes: Vec::new(),
        },
    );
    let world = world();
    for _ in 0..2 {
        match scenes.resolve(world.registry(), AssetId::from_u128(100)) {
            Err(SceneError::TooDeep(chain)) => assert_eq!(chain.len(), 17),
            other => panic!("{other:?}"),
        }
    }
    // Resolved alone first, a scene still nests too deep where another places it.
    scenes
        .resolve(world.registry(), AssetId::from_u128(101))
        .expect("sixteen scenes nest");
    assert!(matches!(
        scenes.resolve(world.registry(), AssetId::from_u128(100)),
        Err(SceneError::TooDeep(_))
    ));
}

#[test]
fn a_world_spawned_from_scene_files_saves_and_loads_back() {
    let mut scenes = library(&[(YARD, "yard", YARD_FILE), (LAMP, "lamp", LAMP_FILE)]);
    let start = |scenes: &mut SceneLibrary| {
        let mut world = world();
        spawn(&mut world, scenes, YARD, "yard");
        world
    };
    let mut played = start(&mut scenes);
    let light = entity(&played, "yard/porch/light");
    played
        .set_property(
            light,
            "test::Glow",
            &"intensity".parse().unwrap(),
            Value::F32(3.0),
        )
        .expect("in range");
    played.despawn(entity(&played, "yard/shed"));
    let save = played
        .save(&start(&mut scenes), &Default::default())
        .expect("saved");
    let mut loaded = start(&mut scenes);
    let report = loaded.load(&save).expect("loaded");
    assert!(report.set_aside.is_empty(), "{:?}", report.set_aside);
    assert!(report.scenes_changed.is_empty());
    assert_eq!(played.first_difference(&loaded), None);
}

#[test]
fn a_scene_that_cannot_spawn_leaves_a_world_that_holds_scenes_as_it_was() {
    let mut scenes = library(&[(YARD, "yard", YARD_FILE), (LAMP, "lamp", LAMP_FILE)]);
    let mut held = world();
    spawn(&mut held, &mut scenes, YARD, "yard");
    let mut same = world();
    spawn(&mut same, &mut scenes, YARD, "yard");
    let unturned = LAMP_FILE.replace(
        "translation = { x = 0.0, y = 1.0, z = 0.0 }",
        "translation = { x = 0.0, y = 1.0, z = 0.0 }, rotation = { x = 0.0, y = 0.0, z = 0.0, w = 2.0 }",
    );
    let mut bad = library(&[(LAMP, "lamp", &unturned)]);
    let scene = bad.resolve(held.registry(), LAMP).expect("resolved");
    assert!(held.spawn_scene("lamp", &scene).is_err());
    assert!(
        held.spawn_scene("yard", &scene).is_err(),
        "the key is taken"
    );
    assert_eq!(held.len(), same.len());
    assert_eq!(held.first_difference(&same), None);
    assert!(held.find(&PersistentId::new(["lamp"])).is_none());
    assert_eq!(
        held.save(&same, &Default::default()).expect("saved"),
        same.save(&same, &Default::default()).expect("saved"),
        "nothing to save: the count of created entities untouched"
    );
}

#[test]
fn values_name_the_assets_of_the_table_by_their_short_names() {
    let mesh = AssetId::from_u128(77);
    let lamp = format!(
        "[assets]\npillar = {{ id = \"{mesh}\" }}\n\n\
         [entities.post.components]\nMeshes = {{ ids = [\"pillar\"], casts_shadows = true }}\n"
    );
    let mut scenes = library(&[(LAMP, "lamp", &lamp)]);
    let mut world = world();
    spawn(&mut world, &mut scenes, LAMP, "lamp");
    assert_eq!(
        world.get::<slime_world::Meshes>(entity(&world, "lamp/post")),
        Some(&slime_world::Meshes {
            ids: vec![mesh],
            casts_shadows: true
        })
    );
    let by_id = lamp.replace("[\"pillar\"]", &format!("[\"{mesh}\"]"));
    let mut scenes = library(&[(LAMP, "lamp", &by_id)]);
    let error = scenes.resolve(world.registry(), LAMP).unwrap_err();
    assert!(error.to_string().contains("no asset is named"), "{error}");
}

/// A world that spawns `text` as the yard's scene file, among `scenes`.
fn respawned(scenes: &mut SceneLibrary, text: &str) -> World {
    scenes.insert_file(YARD, "yard", text);
    let mut world = world();
    spawn(&mut world, scenes, YARD, "yard");
    world
}

/// The persistent identities of the children of the entity `id`, in their order.
fn children_of(world: &World, id: &str) -> Vec<String> {
    world
        .children(entity(world, id))
        .iter()
        .map(|&child| world.persistent_id(child).expect("named").to_string())
        .collect()
}

#[test]
fn a_world_changed_as_an_editor_would_is_written_back_and_spawns_the_same_world() {
    let mut scenes = library(&[(YARD, "yard", YARD_FILE), (LAMP, "lamp", LAMP_FILE)]);
    let mut edited = world();
    let root = spawn(&mut edited, &mut scenes, YARD, "yard");
    // A placed value changed.
    let light = entity(&edited, "yard/gate/light");
    edited
        .set_property(
            light,
            "test::Glow",
            &"tint.g".parse().unwrap(),
            Value::F32(0.25),
        )
        .expect("in range");
    // A placement removed, and one added.
    edited.despawn(entity(&edited, "yard/shed"));
    let lamp = scenes.resolve(edited.registry(), LAMP).expect("resolved");
    let added = edited.spawn_placement(&lamp, Some(root)).expect("placed");
    edited.insert(added, Name("well".to_owned()));
    // An entity of a placement removed, and a component added to one.
    edited.despawn(entity(&edited, "yard/porch/light"));
    edited.insert(entity(&edited, "yard/gate/post"), Hidden);
    // An entity created, given a parent in a placement.
    let hook = edited.spawn();
    edited.insert(hook, Name("hook".to_owned()));
    edited.insert(
        hook,
        Transform::from_translation(glam::DVec3::new(0.0, 0.5, 0.0)),
    );
    edited
        .set_parent(hook, Some(entity(&edited, "yard/gate/post")))
        .expect("alive");
    // Children reordered: the porch after the others.
    let porch = entity(&edited, "yard/porch");
    edited.set_parent(porch, None).expect("alive");
    edited.set_parent(porch, Some(root)).expect("alive");

    let first = edited.write_scene(root).expect("written");
    assert!(first.left_out.is_empty(), "{:?}", first.left_out);
    let again = edited.write_scene(root).expect("written");
    assert_eq!(
        first.text, again.text,
        "created entities keep the IDs that they were given"
    );
    assert!(first.text.contains("prefab = \"lamp\""), "{}", first.text);
    assert!(
        !first.text.contains("intensity = 40.0"),
        "the prefab is placed, not copied: {}",
        first.text
    );
    let mut reread = respawned(&mut scenes, &first.text);
    let second = reread
        .write_scene(entity(&reread, "yard"))
        .expect("written");
    assert_eq!(second.text, first.text, "{}", first.text);
    let third = respawned(&mut scenes, &second.text);
    assert_eq!(reread.first_difference(&third), None);
    // What the editor did holds in the world spawned from the file.
    let children = children_of(&reread, "yard");
    assert_eq!(children.len(), 3);
    assert_eq!(children[0], "yard/gate");
    assert!(children[1].starts_with("yard/well-"), "{children:?}");
    assert_eq!(children[2], "yard/porch");
    let glow = Glow::from_value(&value(&reread, "yard/gate/light", "test::Glow").expect("held"));
    assert_eq!(
        glow.map(|glow| (glow.intensity, glow.tint.g)),
        Ok((10.0, 0.25))
    );
    assert!(reread.find(&"yard/shed".parse().unwrap()).is_none());
    assert!(reread.find(&"yard/porch/light".parse().unwrap()).is_none());
    assert!(value(&reread, "yard/gate/post", "Hidden").is_some());
    let hooks: Vec<String> = children_of(&reread, "yard/gate/post")
        .into_iter()
        .filter(|id| id.starts_with("yard/hook-"))
        .collect();
    assert_eq!(
        hooks.len(),
        1,
        "{:?}",
        children_of(&reread, "yard/gate/post")
    );
    let well = &children[1];
    assert_eq!(
        value(&reread, well, "Name"),
        Some(Name("well".to_owned()).to_value())
    );
    assert!(reread.placement(entity(&reread, well)).is_some());
}

#[test]
fn overrides_that_hold_the_prefabs_value_or_were_set_aside_are_written_back() {
    let yard = format!(
        "{YARD_FILE}\n[changed.\"shed/light\".\"test::Glow\"]\nintensity = 40.0\n\n\
         [added.\"porch/lantern\"]\nHidden = {{}}\n"
    );
    let mut scenes = library(&[(YARD, "yard", &yard), (LAMP, "lamp", LAMP_FILE)]);
    let mut world = world();
    let root = spawn(&mut world, &mut scenes, YARD, "yard");
    let written = world.write_scene(root).expect("written");
    for kept in [
        "[changed.\"shed/light\"]\n\"test::Glow\" = { intensity = 40.0 }\n",
        "[added.\"porch/lantern\"]\nHidden = {}\n",
        "[changed.\"gate/light\"]\n\"test::Glow\" = { intensity = 10.0 }\n",
        "entities = [\"porch/glass\"]",
    ] {
        assert!(written.text.contains(kept), "{kept} in {}", written.text);
    }
    let reread = respawned(&mut scenes, &written.text);
    assert_eq!(world.first_difference(&reread), None);
}

#[test]
fn a_prefab_spawned_alone_and_written_back_gives_itself() {
    let mut scenes = library(&[(LAMP, "lamp", LAMP_FILE)]);
    let mut lamp = world();
    let root = spawn(&mut lamp, &mut scenes, LAMP, "lamp");
    let written = lamp.write_scene(root).expect("written");
    assert_eq!(
        written.text,
        "[entities.post.components]\nName = \"post\"\n\n[entities.post.components.Transform]\n\
         translation = { x = 0.0, y = 1.0, z = 0.0 }\n\n[entities.light]\nparent = \"post\"\n\n\
         [entities.light.components]\n\"test::Aim\" = { at = \"post\" }\n\n\
         [entities.light.components.\"test::Glow\"]\nintensity = 40.0\n\
         tint = { r = 1.0, g = 0.8, b = 0.6 }\n\n[entities.glass]\nparent = \"post\"\n\n\
         [entities.glass.components]\nHidden = {}\n"
    );
    let mut again = library(&[(LAMP, "lamp", &written.text)]);
    let mut reread = world();
    let root = spawn(&mut reread, &mut again, LAMP, "lamp");
    assert_eq!(lamp.first_difference(&reread), None);
    assert_eq!(
        reread.write_scene(root).expect("written").text,
        written.text
    );
}

#[test]
fn what_a_scene_file_cannot_hold_is_reported_and_a_placement_is_written_by_its_scene() {
    let mut scenes = library(&[(YARD, "yard", YARD_FILE), (LAMP, "lamp", LAMP_FILE)]);
    let mut world = world();
    let root = spawn(&mut world, &mut scenes, YARD, "yard");
    spawn(&mut world, &mut scenes, LAMP, "other");
    let sign = world.spawn();
    world.insert(
        sign,
        Aim {
            at: entity(&world, "other/light"),
        },
    );
    world.set_parent(sign, Some(root)).expect("alive");
    world
        .set_parent(entity(&world, "other/glass"), Some(root))
        .expect("alive");
    world
        .set_parent(entity(&world, "yard/gate/light"), Some(root))
        .expect("alive");
    let written = world.write_scene(root).expect("written");
    let left_out: Vec<&str> = written
        .left_out
        .iter()
        .map(|aside| aside.entity.as_str())
        .collect();
    assert_eq!(left_out, ["other/glass", "yard/gate/light", "other/light"]);
    assert!(
        written.text.contains("\"test::Aim\" = { at = \"\" }"),
        "{}",
        written.text
    );
    for id in ["yard/gate", "yard/gate/light"] {
        match world.write_scene(entity(&world, id)) {
            Err(slime_world::DocumentError::NotARoot { .. }) => {}
            other => panic!("{id}: {other:?}"),
        }
    }
}

#[test]
fn an_entity_says_which_placement_it_belongs_to_and_whether_it_differs_from_the_prefab() {
    let mut scenes = library(&[(YARD, "yard", YARD_FILE), (LAMP, "lamp", LAMP_FILE)]);
    let mut world = world();
    spawn(&mut world, &mut scenes, YARD, "yard");
    assert_eq!(
        world.placement_root(entity(&world, "yard/gate/light")),
        Some(entity(&world, "yard/gate"))
    );
    assert_eq!(
        world.placement_root(entity(&world, "yard/gate")),
        Some(entity(&world, "yard/gate"))
    );
    assert_eq!(world.placement_root(entity(&world, "yard")), None);
    let differs =
        |id: &str, component: &str| world.differs_from_prefab(entity(&world, id), component);
    assert_eq!(differs("yard/gate/light", "test::Glow"), Some(true));
    assert_eq!(differs("yard/porch/light", "test::Glow"), Some(false));
    assert_eq!(
        differs("yard/gate", "Name"),
        Some(false),
        "the name that placing gives"
    );
    assert_eq!(differs("yard/gate", "Transform"), Some(true));
    assert_eq!(differs("yard/shed/light", "Hidden"), Some(true), "added");
    assert_eq!(differs("yard/gate/light", "Hidden"), None);
    assert_eq!(differs("yard", "Name"), None);
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
        let root = world.spawn_scene("yard", &scene).expect("spawned");
        let spawned = started.elapsed();
        let started = std::time::Instant::now();
        let written = world.write_scene(root).expect("written");
        let wrote = started.elapsed();
        eprintln!(
            "{placements} placements of {entities} entities: {} entities, resolved in {resolved:?}, spawned in {spawned:?}, written back in {wrote:?}, {} bytes",
            world.len(),
            written.text.len()
        );
        assert_eq!(world.len(), placements * (entities + 1) + 1);
    }
}
