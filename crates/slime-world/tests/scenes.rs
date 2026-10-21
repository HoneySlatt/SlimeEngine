//! Scene files placed in worlds: prefabs and their overrides, nesting, references, the changes of a
//! prefab followed, and files that say something wrong (`docs/adr/0089-scenes-and-prefabs.md`).

use glam::{Mat4, Vec3};
use slime_assets::{AssetId, CookedScene, SceneNode};
use slime_reflect::{Reflect, Value};
use slime_world::{
    Component, Entity, Hidden, Name, PersistentId, SceneError, SceneLibrary, Transform, Unsaved,
    World, WorldError, Written,
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

/// A count, and a cache of it that files leave out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(name = "test::Counter")]
struct Counter {
    count: u32,
    #[reflect(transient, default)]
    cached: u32,
}
impl Component for Counter {}

const YARD: AssetId = AssetId::from_u128(1);
const LAMP: AssetId = AssetId::from_u128(2);
const VILLAGE: AssetId = AssetId::from_u128(3);
const LEVEL: AssetId = AssetId::from_u128(4);
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

/// A blend between two looks, from 0.7 to 0.9: bounds that a 32-bit float does not hold.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Blend", default)]
struct Blend {
    #[reflect(range(min = 0.7, max = 0.9))]
    amount: f32,
}
impl Component for Blend {}

/// At its lower bound, as a game that clamps it to its range sets it.
impl Default for Blend {
    fn default() -> Self {
        Self {
            amount: 0.2_f32.clamp(0.7, 0.9),
        }
    }
}

/// A world that knows the tests' components.
fn world() -> World {
    let mut world = World::new();
    world.register::<Glow>();
    world.register::<Aim>();
    world.register::<Counter>();
    world.settle();
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
fn a_value_at_its_field_s_bound_is_saved_spawned_and_written_back() {
    let blended = |scenes: &mut SceneLibrary| {
        let mut world = world();
        // Its default at its bound, which registering checks.
        world.register::<Blend>();
        let root = spawn(&mut world, scenes, YARD, "yard");
        (world, root)
    };
    let mut scenes = library(&[(
        YARD,
        "yard",
        "[entities.mill.components]\nName = \"mill\"\n\"test::Blend\" = { amount = 0.7 }\n",
    )]);
    let (mut run, root) = blended(&mut scenes);
    let mill = entity(&run, "yard/mill");
    assert_eq!(
        run.get::<Blend>(mill),
        Some(&Blend::default()),
        "a file at the bound"
    );
    run.get_mut::<Blend>(mill).expect("held").amount = 0.9;
    let save = run.save(&Default::default()).expect("saved at the bound");
    let (mut loaded, _) = blended(&mut scenes);
    loaded
        .load(&save, &mut scenes, &mut |_, _| {})
        .expect("loaded");
    assert_eq!(
        loaded.get::<Blend>(entity(&loaded, "yard/mill")),
        Some(&Blend { amount: 0.9 })
    );
    run.get_mut::<Blend>(mill).expect("held").amount = 0.7;
    let written = run.write_scene(root).expect("written at the bound");
    scenes.insert_file(YARD, "yard", written.text);
    let (again, _) = blended(&mut scenes);
    assert_eq!(
        again.get::<Blend>(entity(&again, "yard/mill")),
        Some(&Blend::default())
    );
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
        .map(|child| yard.persistent_id(child).expect("named").to_string())
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
        yard.is_hidden(entity(&yard, "yard/shed/light")),
        "hidden by the yard"
    );
    assert!(
        yard.is_hidden(entity(&yard, "yard/gate/glass")),
        "hidden by the lamp"
    );
    assert!(!yard.is_hidden(entity(&yard, "yard/gate/light")));
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
        components: Vec::new(),
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
fn a_scene_whose_placements_copy_more_than_the_limit_is_refused_before_any_is_made() {
    // A lamp of 512 kB, placed 2 000 times by a file of 60 kB: 1 GB of copies, far fewer entities
    // than the limit.
    let lamp = format!(
        "[entities.post.components]\nName = \"{}\"\n",
        "a".repeat(512 << 10)
    );
    let mut scenes = library(&[
        (YARD, "yard", &many_placements(2_000)),
        (LAMP, "lamp", &lamp),
    ]);
    let world = world();
    match scenes.resolve(world.registry(), YARD) {
        Err(SceneError::TooLarge { scene }) => assert_eq!(scene, "yard"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_placement_whose_id_each_copy_repeats_counts_it_in_the_limit() {
    // A prefab of 1 000 entities placed once under an ID of 4 MiB, which each entity's path, and
    // the key that names it while the scene resolves, repeat: 8 GB of copies, from a file of 4 MiB.
    let yard = format!(
        "[assets]\nlamp = {{ id = \"{LAMP}\" }}\n\n[entities.{}]\nprefab = \"lamp\"\n",
        "a".repeat(4 << 20)
    );
    let mut scenes = library(&[(YARD, "yard", &yard), (LAMP, "lamp", &chain_prefab(1_000))]);
    match scenes.resolve(world().registry(), YARD) {
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
        world.settle();
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
    let save = played.save(&Default::default()).expect("saved");
    let mut loaded = start(&mut scenes);
    let report = loaded
        .load(&save, &mut scenes, &mut |_, _| {})
        .expect("loaded");
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
        held.save(&Default::default()).expect("saved"),
        same.save(&Default::default()).expect("saved"),
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
    world.settle();
    world
}

/// The persistent identities of the children of the entity `id`, in their order.
fn children_of(world: &World, id: &str) -> Vec<String> {
    world
        .children(entity(world, id))
        .map(|child| world.persistent_id(child).expect("named").to_string())
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
    // What the editor did holds in the world spawned from the file, whose created entities have
    // the identities that the file gives them.
    assert_eq!(edited.len(), reread.len());
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

/// Writes the yard of `world` back, twice, which gives the same bytes, and spawns the file again
/// among `scenes`: the file, and the world spawned from it.
fn written_back(world: &mut World, scenes: &mut SceneLibrary) -> (Written, World) {
    let root = entity(world, "yard");
    let written = world.write_scene(root).expect("written");
    assert_eq!(world.write_scene(root).expect("written").text, written.text);
    let reread = respawned(scenes, &written.text);
    (written, reread)
}

#[test]
fn an_entity_that_a_key_names_and_another_scene_under_a_level_written_back_are_left_out_as_such() {
    let mut scenes = library(&[(YARD, "yard", YARD_FILE), (LAMP, "lamp", LAMP_FILE)]);
    let mut world = world();
    let root = spawn(&mut world, &mut scenes, YARD, "yard");
    let camera = world.spawn_keyed("camera").expect("a free key");
    world.set_parent(camera, Some(root)).expect("alive");
    // And the root of another scene, whose identity is one part too.
    let other = spawn(&mut world, &mut scenes, LAMP, "other");
    world.set_parent(other, Some(root)).expect("alive");
    let written = world.write_scene(root).expect("written");
    let reason = |entity: &str| {
        let aside = written.left_out.iter().find(|aside| aside.entity == entity);
        aside.map_or("", |aside| aside.reason.as_str())
    };
    assert!(
        reason("camera").contains("a key names it"),
        "{:?}",
        written.left_out
    );
    assert!(
        reason("other").contains("another scene"),
        "{:?}",
        written.left_out
    );
    assert_eq!(written.left_out.len(), 2, "{:?}", written.left_out);
}

/// The entities and components of what a written file left out.
fn left_out(written: &Written) -> Vec<(&str, Option<&str>)> {
    written
        .left_out
        .iter()
        .map(|aside| (aside.entity.as_str(), aside.component.as_deref()))
        .collect()
}

#[test]
fn changes_that_the_file_wrote_whose_target_the_world_removed_are_not_written_again() {
    for removed in ["the component", "the entity", "its ancestor"] {
        let mut scenes = library(&[(YARD, "yard", YARD_FILE), (LAMP, "lamp", LAMP_FILE)]);
        let mut edited = world();
        spawn(&mut edited, &mut scenes, YARD, "yard");
        let light = entity(&edited, "yard/gate/light");
        match removed {
            "the component" => {
                edited.remove::<Glow>(light);
            }
            "the entity" => {
                edited.despawn(light);
            }
            _ => {
                edited.despawn(entity(&edited, "yard/gate/post"));
            }
        }
        let (written, reread) = written_back(&mut edited, &mut scenes);
        assert!(
            !written.text.contains("[changed.\"gate/light\"]"),
            "{removed}: {}",
            written.text
        );
        assert_eq!(left_out(&written), [], "{removed}");
        let scene = scenes.resolve(reread.registry(), YARD).expect("resolved");
        assert_eq!(scene.set_aside(), [], "{removed}");
        assert_eq!(edited.first_difference(&reread), None, "{removed}");
    }
}

#[test]
fn overrides_that_the_file_wrote_follow_what_the_world_did_since() {
    let lamp = format!(
        "{LAMP_FILE}\n[entities.bulb]\nparent = \"post\"\n\n[entities.bulb.components]\n\
         Light = {{ Point = {{ color = {{ r = 1.0, g = 0.8, b = 0.6, a = 1.0 }}, \
         intensity = 40.0, range = 8.0 }} }}\n"
    );
    // A component added, one removed twice, and a part of a variant that the prefab's bulb does
    // not hold, set aside.
    let yard = format!(
        "{YARD_FILE}\n[added.\"porch/post\".\"test::Glow\"]\nintensity = 5.0\n\
         tint = {{ r = 0.0, g = 0.0, b = 0.0 }}\n\n[removed.components]\n\
         \"gate/glass\" = [\"Hidden\"]\n\"shed/glass\" = [\"Hidden\"]\n\n\
         [changed.\"gate/bulb\".Light.Spot]\nintensity = 10.0\n"
    );
    let mut scenes = library(&[(YARD, "yard", &yard), (LAMP, "lamp", &lamp)]);
    let mut edited = world();
    let set_aside = scenes
        .resolve(edited.registry(), YARD)
        .expect("resolved")
        .set_aside()
        .to_vec();
    spawn(&mut edited, &mut scenes, YARD, "yard");
    assert_eq!(set_aside.len(), 1, "{set_aside:?}");
    // The component added is edited, and one removed is given back.
    edited
        .set_property(
            entity(&edited, "yard/porch/post"),
            "test::Glow",
            &"intensity".parse().unwrap(),
            Value::F32(7.0),
        )
        .expect("in range");
    edited.insert(entity(&edited, "yard/gate/glass"), Hidden);
    let (written, reread) = written_back(&mut edited, &mut scenes);
    assert_eq!(left_out(&written), []);
    for kept in [
        "intensity = 7.0",
        "\"shed/glass\" = [\"Hidden\"]",
        "[changed.\"gate/bulb\".Light]\nSpot = { intensity = 10.0 }\n",
    ] {
        assert!(written.text.contains(kept), "{kept} in {}", written.text);
    }
    assert!(
        !written.text.contains("\"gate/glass\" = [\"Hidden\"]"),
        "{}",
        written.text
    );
    assert_eq!(edited.first_difference(&reread), None);
    let scene = scenes.resolve(reread.registry(), YARD).expect("resolved");
    assert_eq!(scene.set_aside(), set_aside);

    // The bulb changed since: the part set aside cannot say it, and gives way to the value.
    let bulb = entity(&edited, "yard/gate/bulb");
    let Some(Value::Enum(variant, mut fields)) = edited.component_value(bulb, "Light") else {
        panic!("a light");
    };
    for (field, value) in &mut fields {
        if field == "intensity" {
            *value = Value::F32(20.0);
        }
    }
    edited
        .set_component_value(bulb, "Light", Value::Enum(variant, fields))
        .expect("a light");
    let (written, reread) = written_back(&mut edited, &mut scenes);
    assert_eq!(left_out(&written), [("yard/gate/bulb", Some("Light"))]);
    assert!(
        written
            .text
            .contains("[changed.\"gate/bulb\".Light]\nPoint = { intensity = 20.0 }\n"),
        "{}",
        written.text
    );
    assert_eq!(edited.first_difference(&reread), None);
}

#[test]
fn an_entity_whose_id_a_file_cannot_hold_or_that_another_has_now_is_written_under_another() {
    // A node of a glTF scene without a name of its own, whose ID, `#1`, a file cannot hold.
    let node = |name: &str, parent: Option<u32>| SceneNode {
        name: name.to_owned(),
        parent,
        transform: Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)).to_cols_array(),
        meshes: vec![AssetId::from_u128(7)],
        light: None,
        extras: Vec::new(),
        components: Vec::new(),
    };
    let chapel = CookedScene {
        name: "chapel".to_owned(),
        nodes: vec![node("nave", None), node("", Some(0))],
    };
    let mut scenes = SceneLibrary::new();
    scenes.insert_cooked(CHAPEL, chapel);
    let mut zone = world();
    let root = spawn(&mut zone, &mut scenes, CHAPEL, "zone");
    let written = zone.write_scene(root).expect("written");
    assert_eq!(left_out(&written), [("zone/#1", None)]);
    scenes.insert_file(LEVEL, "zone", written.text.clone());
    let mut reread = world();
    let root = spawn(&mut reread, &mut scenes, LEVEL, "zone");
    assert_eq!(reread.len(), zone.len());
    let rewritten = reread.write_scene(root).expect("written");
    assert_eq!(rewritten.text, written.text);
    assert_eq!(left_out(&rewritten), []);

    // An entity created outside a scene keeps the ID that it was first written under while that
    // is free; once the file is spawned again, the entity that the file makes has it.
    let mut editor = world();
    let root = editor.spawn();
    let hook = editor.spawn();
    editor.insert(hook, Name("hook".to_owned()));
    editor.set_parent(hook, Some(root)).expect("alive");
    let first = editor.write_scene(root).expect("written");
    let mut scenes = library(&[(LEVEL, "level", &first.text)]);
    let level = spawn(&mut editor, &mut scenes, LEVEL, "level");
    editor.remove::<Name>(hook);
    editor.insert(hook, Hidden);
    editor.set_parent(hook, Some(level)).expect("alive");
    let written = editor.write_scene(level).expect("written");
    let mut scenes = library(&[(LEVEL, "level", &written.text)]);
    let mut reread = world();
    let level = spawn(&mut reread, &mut scenes, LEVEL, "level");
    let children: Vec<String> = reread
        .children(level)
        .map(|child| reread.persistent_id(child).expect("named").to_string())
        .collect();
    assert_eq!(children.len(), 2, "{}", written.text);
    assert!(children[0].starts_with("level/hook-"), "{children:?}");
    assert!(children[1].starts_with("level/entity-"), "{children:?}");
}

#[test]
fn a_rotation_changed_is_written_whole_and_survives_its_prefab_turned() {
    let mut scenes = library(&[(YARD, "yard", YARD_FILE), (LAMP, "lamp", LAMP_FILE)]);
    let mut edited = world();
    spawn(&mut edited, &mut scenes, YARD, "yard");
    let post = entity(&edited, "yard/gate/post");
    let mut transform = *edited.get::<Transform>(post).expect("held");
    transform.rotation = glam::Quat::from_rotation_y(0.5);
    edited.insert(post, transform);
    let (written, _) = written_back(&mut edited, &mut scenes);
    let rotation = transform.rotation;
    let whole = format!(
        "[changed.\"gate/post\".Transform]\nrotation = {{ x = 0.0, y = {:?}, z = 0.0, w = {:?} }}\n",
        rotation.y, rotation.w
    );
    assert!(written.text.contains(&whole), "{}", written.text);
    // The prefab's post turns about X: the file's rotation replaces it, whole.
    let turned = LAMP_FILE.replace(
        "Transform = { translation = { x = 0.0, y = 1.0, z = 0.0 } }",
        "Transform = { translation = { x = 0.0, y = 1.0, z = 0.0 }, \
         rotation = { x = 0.38268343, y = 0.0, z = 0.0, w = 0.9238795 } }",
    );
    scenes.insert_file(LAMP, "lamp", turned);
    let reread = respawned(&mut scenes, &written.text);
    let post = reread
        .get::<Transform>(entity(&reread, "yard/gate/post"))
        .expect("held");
    assert_eq!(post.rotation, rotation);
}

#[test]
fn what_a_file_cannot_say_of_a_root_or_of_a_placements_entities_is_reported() {
    let written = |edit: &dyn Fn(&mut World)| {
        let mut scenes = library(&[(YARD, "yard", YARD_FILE), (LAMP, "lamp", LAMP_FILE)]);
        let mut edited = world();
        spawn(&mut edited, &mut scenes, YARD, "yard");
        edit(&mut edited);
        edited
            .write_scene(entity(&edited, "yard"))
            .expect("written")
    };
    let move_last = |world: &mut World, id: &str| {
        let moved = entity(world, id);
        let parent = world.parent(moved);
        world.set_parent(moved, None).expect("alive");
        world.set_parent(moved, parent).expect("alive");
    };
    // The root without what spawning gives it.
    let without_name = written(&|world| {
        world.remove::<Name>(entity(world, "yard"));
    });
    assert_eq!(left_out(&without_name), [("yard", Some("Name"))]);
    let without_transform = written(&|world| {
        world.remove::<Transform>(entity(world, "yard"));
    });
    assert_eq!(left_out(&without_transform), [("yard", Some("Transform"))]);
    // The children of a placement's entity reordered: its light last, an entity created first.
    let reordered = written(&|world| {
        let post = entity(world, "yard/gate/post");
        let hook = world.spawn();
        world.insert(hook, Name("hook".to_owned()));
        world.set_parent(hook, Some(post)).expect("alive");
        move_last(world, "yard/gate/glass");
        move_last(world, "yard/gate/light");
    });
    assert_eq!(left_out(&reordered), [("yard/gate/post", None)]);
    // A placement's entity moved out, first among the root's children, an entity created under
    // it: written in its place, with what is under it.
    let moved = written(&|world| {
        let root = entity(world, "yard");
        let light = entity(world, "yard/gate/light");
        world.set_parent(light, Some(root)).expect("alive");
        for id in ["yard/porch", "yard/gate", "yard/shed"] {
            move_last(world, id);
        }
        let hook = world.spawn();
        world.insert(hook, Name("hook".to_owned()));
        world.set_parent(hook, Some(light)).expect("alive");
    });
    assert_eq!(left_out(&moved), [("yard/gate/light", None)]);
    assert!(
        moved.text.contains("parent = \"gate/light\""),
        "{}",
        moved.text
    );
    // Moved out of a place that is removed since: lost, and what the file wrote of it with it.
    let lost = written(&|world| {
        let root = entity(world, "yard");
        world
            .set_parent(entity(world, "yard/gate/light"), Some(root))
            .expect("alive");
        world.despawn(entity(world, "yard/gate/post"));
    });
    assert_eq!(left_out(&lost), [("yard/gate/light", None)]);
    assert!(
        lost.left_out[0].reason.contains("lost"),
        "{:?}",
        lost.left_out
    );
    assert!(
        !lost.text.contains("[changed.\"gate/light\"]"),
        "{}",
        lost.text
    );
}

#[test]
fn a_level_written_back_unchanged_keeps_what_its_file_marks_unsaved() {
    // A campfire whose smoke, made again at each run, saves leave out, with an ember under it; a
    // lamp placed, marked the same way; the root marked too.
    let zone = r#"
[assets]
lamp = { id = "00000000-0000-0000-0000-000000000002", path = "lamp.scene" }

[root.components]
Unsaved = {}

[entities.campfire.components]
Name = "campfire"

[entities.smoke]
parent = "campfire"

[entities.smoke.components]
Unsaved = {}

[entities.ember]
parent = "smoke"

[entities.ember.components]
Name = "ember"

[entities.lantern]
prefab = "lamp"

[added.lantern]
Unsaved = {}
"#;
    let mut scenes = library(&[(YARD, "yard", zone), (LAMP, "lamp", LAMP_FILE)]);
    let mut world = world();
    let root = spawn(&mut world, &mut scenes, YARD, "yard");
    // A spark that the run made under the smoke, which neither a save nor the file keeps.
    let spark = world.spawn();
    world
        .set_parent(spark, Some(entity(&world, "yard/smoke")))
        .expect("alive");
    let written = world.write_scene(root).expect("written");
    assert_eq!(left_out(&written), [("created/1", None)]);
    let again = respawned(&mut scenes, &written.text);
    for id in [
        "yard/smoke",
        "yard/ember",
        "yard/lantern",
        "yard/lantern/light",
    ] {
        assert!(
            again.find(&id.parse().unwrap()).is_some(),
            "{id}: {}",
            written.text
        );
    }
    for id in ["yard", "yard/smoke", "yard/lantern"] {
        assert!(again.get::<Unsaved>(entity(&again, id)).is_some(), "{id}");
    }
    assert_eq!(again.len() + 1, world.len());
    let rewritten = respawned(&mut scenes, &written.text)
        .write_scene(entity(&again, "yard"))
        .expect("written");
    assert_eq!(rewritten.text, written.text);
}

#[test]
fn unsaved_is_written_but_what_was_created_with_it_and_transient_fields_are_left_out() {
    let mut scenes = library(&[(YARD, "yard", YARD_FILE), (LAMP, "lamp", LAMP_FILE)]);
    let mut edited = world();
    let root = spawn(&mut edited, &mut scenes, YARD, "yard");
    // A placement's entity marked Unsaved, changed since: written as it is.
    let light = entity(&edited, "yard/gate/light");
    edited.insert(light, Unsaved);
    edited
        .set_property(
            light,
            "test::Glow",
            &"intensity".parse().unwrap(),
            Value::F32(3.0),
        )
        .expect("in range");
    // An entity created and marked Unsaved, which a placed value refers to.
    let helper = edited.spawn();
    edited.insert(helper, Unsaved);
    edited.set_parent(helper, Some(root)).expect("alive");
    edited.insert(entity(&edited, "yard/porch/light"), Aim { at: helper });
    // A component whose cache files leave out.
    edited.insert(
        entity(&edited, "yard/gate/post"),
        Counter {
            count: 3,
            cached: 9,
        },
    );
    let written = edited.write_scene(root).expect("written");
    for kept in [
        "[added.\"gate/light\"]\nUnsaved = {}\n",
        "[changed.\"gate/light\"]\n\"test::Glow\" = { intensity = 3.0 }\n",
        "\"test::Counter\" = { count = 3 }",
        "[changed.\"porch/light\"]\n\"test::Aim\" = { at = \"\" }\n",
    ] {
        assert!(written.text.contains(kept), "{kept} in {}", written.text);
    }
    // The helper, left out, and the reference to it.
    assert_eq!(left_out(&written), [("created/1", None); 2]);
    assert!(
        written
            .left_out
            .iter()
            .all(|aside| aside.reason.contains("Unsaved")),
        "{:?}",
        written.left_out
    );
    assert_eq!(
        respawned(&mut scenes, &written.text).first_difference(&edited),
        None,
        "{}",
        written.text
    );
}

#[test]
fn a_placement_made_while_the_world_runs_is_made_whole_or_not_at_all() {
    // A prefab whose value breaks a rule of its type's check, which resolving leaves to the world.
    let broken = "[entities.post.components]\nName = \"post\"\n\n[entities.light]\nparent = \"post\"\n\n\
                  [entities.light.components]\n\
                  Transform = { rotation = { x = 1.0, y = 1.0, z = 0.0, w = 0.0 } }\n";
    let mut scenes = library(&[(VILLAGE, "broken", broken), (LAMP, "lamp", LAMP_FILE)]);
    let mut world = world();
    let parent = world.spawn();
    let prefab = scenes.resolve(world.registry(), VILLAGE).expect("resolved");
    let before = world.len();
    assert!(matches!(
        world.spawn_placement(&prefab, Some(parent)),
        Err(WorldError::RefusedInScene { .. })
    ));
    let lamp = scenes.resolve(world.registry(), LAMP).expect("resolved");
    let mut bare = World::new();
    assert!(matches!(
        bare.spawn_placement(&lamp, None),
        Err(WorldError::UnknownComponent(_))
    ));
    assert_eq!(bare.len(), 0);
    assert_eq!(world.len(), before);
    assert_eq!(world.children(parent).len(), 0);
    let next = world.spawn();
    assert_eq!(
        world.persistent_id(next).map(|id| id.to_string()),
        Some("created/2".to_owned()),
        "a placement refused takes no number"
    );
    let placed = world.spawn_placement(&lamp, Some(parent)).expect("placed");
    assert_eq!(
        world.persistent_id(placed).map(|id| id.to_string()),
        Some("created/3".to_owned())
    );
    assert_eq!(world.parent(placed), Some(parent));
    assert_eq!(world.get::<Name>(placed), None);
    let light = entity(&world, "created/3/light");
    assert_eq!(world.placement_root(light), Some(placed));
    assert_eq!(world.differs_from_prefab(light, "test::Glow"), Some(false));
    assert!(world.despawn(placed));
    assert!(world.find(&"created/3/light".parse().unwrap()).is_none());
}

#[test]
fn placements_made_while_the_world_runs_are_written_as_placements_of_their_prefabs() {
    let mut scenes = library(&[(YARD, "yard", YARD_FILE), (LAMP, "lamp", LAMP_FILE)]);
    // Another prefab, whose scene is named as the lamp's.
    scenes.insert_file(VILLAGE, "lamp", LAMP_FILE);
    let mut edited = world();
    let root = spawn(&mut edited, &mut scenes, YARD, "yard");
    let lamp = scenes.resolve(edited.registry(), LAMP).expect("resolved");
    let other = scenes
        .resolve(edited.registry(), VILLAGE)
        .expect("resolved");
    edited.spawn_placement(&lamp, Some(root)).expect("placed");
    let named = edited.spawn_placement(&other, Some(root)).expect("placed");
    edited.insert(named, Name("other".to_owned()));
    let (written, mut reread) = written_back(&mut edited, &mut scenes);
    assert_eq!(left_out(&written), []);
    let children = children_of(&reread, "yard");
    assert_eq!(children.len(), 5, "{children:?}");
    let unnamed = children[3].strip_prefix("yard/").expect("in the yard");
    assert!(unnamed.starts_with("lamp-"), "{children:?}");
    assert!(children[4].starts_with("yard/other-"), "{children:?}");
    for kept in [
        format!("{unnamed} = {{ prefab = \"lamp\" }}"),
        format!("{unnamed} = [\"Name\"]"),
        "\"lamp 2\" = { id = \"00000000-0000-0000-0000-000000000003\" }".to_owned(),
    ] {
        assert!(written.text.contains(&kept), "{kept} in {}", written.text);
    }
    assert_eq!(edited.len(), reread.len());
    let second = reread
        .write_scene(entity(&reread, "yard"))
        .expect("written");
    assert_eq!(second.text, written.text);
    let third = respawned(&mut scenes, &second.text);
    assert_eq!(reread.first_difference(&third), None);
}

#[test]
fn a_level_made_from_nothing_is_written_and_spawned_under_its_name() {
    let mut scenes = library(&[(LAMP, "lamp", LAMP_FILE)]);
    let mut made = world();
    let root = made.spawn();
    made.insert(root, Name("level".to_owned()));
    let lamp = scenes.resolve(made.registry(), LAMP).expect("resolved");
    let placed = made.spawn_placement(&lamp, Some(root)).expect("placed");
    made.insert(placed, Name("lamp".to_owned()));
    let first = made.write_scene(root).expect("written");
    assert!(
        first
            .text
            .contains("[root]\ncomponents = { Name = \"level\" }\n"),
        "{}",
        first.text
    );
    scenes.insert_file(LEVEL, "level", first.text.clone());
    let mut reread = world();
    let level = spawn(&mut reread, &mut scenes, LEVEL, "level");
    // Spawned under its name, the root's name is the one that spawning gives it.
    let second = reread.write_scene(level).expect("written");
    assert_eq!(
        second.text,
        first
            .text
            .replace("[root]\ncomponents = { Name = \"level\" }\n\n", "")
    );
    scenes.insert_file(LEVEL, "level", second.text.clone());
    let mut third = world();
    let level = spawn(&mut third, &mut scenes, LEVEL, "level");
    assert_eq!(reread.first_difference(&third), None);
    assert_eq!(third.write_scene(level).expect("written").text, second.text);
}

#[test]
fn nested_placements_edited_two_deep_are_written_back() {
    let village = format!(
        "[assets]\nlamp = {{ id = \"{LAMP}\" }}\n\n[entities.square]\nprefab = \"lamp\"\n\n\
         [entities.well]\nprefab = \"lamp\"\n\n[changed.\"square/light\".\"test::Glow\"]\n\
         intensity = 20.0\ntint = {{ r = 0.5 }}\n\n[removed]\nentities = [\"well/glass\"]\n"
    );
    let yard = format!(
        "[assets]\nvillage = {{ id = \"{VILLAGE}\" }}\n\n[entities.north]\nprefab = \"village\"\n\n\
         [entities.south]\nprefab = \"village\"\n\n[changed.\"north/square/light\".\"test::Glow\"]\n\
         intensity = 30.0\n\n[added.\"south/well/post\"]\n\"test::Aim\" = {{ at = \"north/square/post\" }}\n"
    );
    let mut scenes = library(&[
        (YARD, "yard", &yard),
        (VILLAGE, "village", &village),
        (LAMP, "lamp", LAMP_FILE),
    ]);
    let mut edited = world();
    spawn(&mut edited, &mut scenes, YARD, "yard");
    edited
        .set_property(
            entity(&edited, "yard/north/square/light"),
            "test::Glow",
            &"tint.b".parse().unwrap(),
            Value::F32(0.125),
        )
        .expect("in range");
    edited.despawn(entity(&edited, "yard/north/well/light"));
    edited.insert(
        entity(&edited, "yard/south/square/glass"),
        Counter {
            count: 1,
            cached: 0,
        },
    );
    edited.remove::<Hidden>(entity(&edited, "yard/north/square/glass"));
    // A value two deep refers to an entity created under another placement.
    let hook = edited.spawn();
    edited.insert(hook, Name("hook".to_owned()));
    edited
        .set_parent(hook, Some(entity(&edited, "yard/north/square/post")))
        .expect("alive");
    edited.insert(entity(&edited, "yard/south/square/light"), Aim { at: hook });
    let (written, mut reread) = written_back(&mut edited, &mut scenes);
    assert_eq!(left_out(&written), []);
    assert_eq!(edited.len(), reread.len());
    let glow =
        Glow::from_value(&value(&reread, "yard/north/square/light", "test::Glow").expect("held"))
            .expect("a glow");
    assert_eq!(
        (glow.intensity, glow.tint.r, glow.tint.b),
        (30.0, 0.5, 0.125)
    );
    assert!(
        reread
            .find(&"yard/north/well/light".parse().unwrap())
            .is_none()
    );
    assert!(value(&reread, "yard/south/square/glass", "test::Counter").is_some());
    assert!(value(&reread, "yard/north/square/glass", "Hidden").is_none());
    assert!(value(&reread, "yard/south/well/post", "test::Aim").is_some());
    let aim = reread
        .get::<Aim>(entity(&reread, "yard/south/square/light"))
        .expect("held");
    let target = reread.persistent_id(aim.at).expect("named").to_string();
    assert!(target.starts_with("yard/hook-"), "{target}");
    assert_eq!(
        parent(&reread, &target),
        Some("yard/north/square/post".to_owned())
    );
    let second = reread
        .write_scene(entity(&reread, "yard"))
        .expect("written");
    assert_eq!(second.text, written.text);
}

/// A village that places the lamp twice, the well's without its glass.
const VILLAGE_FILE: &str = r#"
[assets]
lamp = { id = "00000000-0000-0000-0000-000000000002" }

[entities.square]
prefab = "lamp"

[entities.well]
prefab = "lamp"

[removed]
entities = ["well/glass"]
"#;

/// A library of the yard, the lamp and the village.
fn yard_lamp_and_village() -> SceneLibrary {
    library(&[
        (YARD, "yard", YARD_FILE),
        (LAMP, "lamp", LAMP_FILE),
        (VILLAGE, "village", VILLAGE_FILE),
    ])
}

/// The yard spawned in a new world, as a game makes its start world.
fn yard(scenes: &mut SceneLibrary) -> World {
    let mut world = world();
    spawn(&mut world, scenes, YARD, "yard");
    world.settle();
    world
}

/// A run of the yard: a lamp placed under the porch, its light dimmed and its glass removed; the
/// village spawned under a key, a post hidden and a light's aim removed; an entity created under
/// the lamp's post, which aims at a light of the village.
fn run(scenes: &mut SceneLibrary) -> World {
    let mut run = yard(scenes);
    let lamp = scenes.resolve(run.registry(), LAMP).expect("resolved");
    run.spawn_placement(&lamp, Some(entity(&run, "yard/porch")))
        .expect("placed");
    run.set_property(
        entity(&run, "created/1/light"),
        "test::Glow",
        &"intensity".parse().unwrap(),
        Value::F32(5.0),
    )
    .expect("in range");
    run.despawn(entity(&run, "created/1/glass"));
    let village = scenes.resolve(run.registry(), VILLAGE).expect("resolved");
    run.spawn_scene("village", &village).expect("spawned");
    run.insert(entity(&run, "village/square/post"), Hidden);
    run.remove::<Aim>(entity(&run, "village/well/light"));
    let hook = run.spawn();
    run.insert(hook, Name("hook".to_owned()));
    run.insert(
        hook,
        Aim {
            at: entity(&run, "village/square/light"),
        },
    );
    run.set_parent(hook, Some(entity(&run, "created/1/post")))
        .expect("alive");
    run
}

/// The intensity of the glow of the entity `id`.
fn intensity(world: &World, id: &str) -> f32 {
    Glow::from_value(&value(world, id, "test::Glow").expect("held"))
        .expect("a glow")
        .intensity
}

#[test]
fn what_a_run_spawns_is_saved_as_its_scenes_and_loads_back_into_the_same_world() {
    let mut scenes = yard_lamp_and_village();
    let played = run(&mut scenes);
    let save = played.save(&Default::default()).expect("saved");
    for written in [
        "layout = 5\n".to_owned(),
        format!("\"created/1\" = {{ scene = \"{LAMP}\", name = \"lamp\", content = "),
        format!("village = {{ scene = \"{VILLAGE}\", name = \"village\", content = "),
        "[changed.\"created/1/light\"]\n\"test::Glow\" = { intensity = 5.0 }\n".to_owned(),
        "\"created/1\" = [\"yard/porch\"]".to_owned(),
    ] {
        assert!(save.contains(&written), "{written} in {save}");
    }
    assert!(
        !save.contains("[created.\"created/1\"]") && !save.contains("[created.\"village"),
        "placed, not created: {save}"
    );
    let mut loaded = yard(&mut scenes);
    let report = loaded
        .load(&save, &mut scenes, &mut |_, _| {})
        .expect("loaded");
    assert!(
        report.set_aside.is_empty() && report.scenes_changed.is_empty(),
        "{report:?}"
    );
    assert_eq!(played.first_difference(&loaded), None, "{save}");
    assert_eq!(loaded.save(&Default::default()).expect("saved again"), save);
    assert!(loaded.placement(entity(&loaded, "created/1")).is_some());
    let hook = loaded
        .get::<Aim>(entity(&loaded, "created/2"))
        .expect("held");
    assert_eq!(hook.at, entity(&loaded, "village/square/light"));
    let next = loaded.spawn();
    assert_eq!(
        loaded.persistent_id(next).map(|id| id.to_string()),
        Some("created/3".to_owned())
    );
}

#[test]
fn a_prefab_fixed_after_a_save_reaches_what_the_run_spawned_but_what_the_run_changed() {
    let mut scenes = yard_lamp_and_village();
    let mut played = run(&mut scenes);
    let lamp = scenes.resolve(played.registry(), LAMP).expect("resolved");
    played.spawn_placement(&lamp, None).expect("placed");
    let save = played.save(&Default::default()).expect("saved");
    // The lamp glows more, and holds a hook of its own.
    let fixed = LAMP_FILE.replace("intensity = 40.0", "intensity = 50.0")
        + "\n[entities.hook]\nparent = \"post\"\n";
    scenes.insert_file(LAMP, "lamp", fixed);
    let mut loaded = yard(&mut scenes);
    let report = loaded
        .load(&save, &mut scenes, &mut |_, _| {})
        .expect("loaded");
    assert!(report.set_aside.is_empty(), "{:?}", report.set_aside);
    let mut changed = report.scenes_changed.clone();
    changed.sort();
    assert_eq!(
        changed,
        ["created/1", "created/3", "village", "yard"],
        "the scenes that place the lamp are named too"
    );
    for (id, expected) in [
        ("created/3/light", 50.0),
        ("created/1/light", 5.0),
        ("village/square/light", 50.0),
        ("yard/porch/light", 50.0),
        ("yard/gate/light", 10.0),
    ] {
        assert_eq!(intensity(&loaded, id), expected, "{id}");
    }
    for id in [
        "created/1/hook",
        "created/3/hook",
        "village/well/hook",
        "yard/porch/hook",
    ] {
        assert!(loaded.find(&id.parse().unwrap()).is_some(), "{id}");
    }
    assert!(loaded.find(&"created/1/glass".parse().unwrap()).is_none());
}

#[test]
fn what_a_run_spawned_from_a_scene_that_the_game_no_longer_has_is_set_aside() {
    let mut scenes = yard_lamp_and_village();
    let mut played = run(&mut scenes);
    // A village placed as a prefab too, which goes with it.
    let village = scenes
        .resolve(played.registry(), VILLAGE)
        .expect("resolved");
    played.spawn_placement(&village, None).expect("placed");
    let save = played.save(&Default::default()).expect("saved");
    let mut without = library(&[(YARD, "yard", YARD_FILE), (LAMP, "lamp", LAMP_FILE)]);
    let mut loaded = yard(&mut without);
    let report = loaded
        .load(&save, &mut without, &mut |_, _| {})
        .expect("loaded");
    assert!(loaded.find(&"village".parse().unwrap()).is_none());
    for (entity, component) in [
        ("village", None),
        ("created/3", None),
        ("village/square/post", Some("Hidden")),
        ("village/well/light", Some("test::Aim")),
        ("created/2", Some("test::Aim")),
    ] {
        assert!(
            report
                .set_aside
                .iter()
                .any(|aside| aside.entity == entity && aside.component.as_deref() == component),
            "{entity}: {:?}",
            report.set_aside
        );
    }
    let hook = loaded
        .get::<Aim>(entity(&loaded, "created/2"))
        .expect("held");
    assert!(!loaded.contains(hook.at), "it aims at no entity");
    assert_eq!(intensity(&loaded, "created/1/light"), 5.0);
}

#[test]
fn a_placement_whose_root_is_destroyed_is_saved_while_an_entity_of_it_lives() {
    let mut scenes = yard_lamp_and_village();
    let mut played = yard(&mut scenes);
    let lamp = scenes.resolve(played.registry(), LAMP).expect("resolved");
    let placed = played.spawn_placement(&lamp, None).expect("placed");
    // Its light moved into the yard, then the lamp destroyed.
    played
        .set_parent(
            entity(&played, "created/1/light"),
            Some(entity(&played, "yard")),
        )
        .expect("alive");
    played.despawn(placed);
    // A lamp marked Unsaved, which a save leaves out.
    let unsaved = played.spawn_placement(&lamp, None).expect("placed");
    played.insert(unsaved, slime_world::Unsaved);
    let save = played.save(&Default::default()).expect("saved");
    assert!(save.contains("\"created/1\" = { scene"), "{save}");
    assert!(!save.contains("created/2"), "{save}");
    let mut loaded = yard(&mut scenes);
    let report = loaded
        .load(&save, &mut scenes, &mut |_, _| {})
        .expect("loaded");
    assert!(report.set_aside.is_empty(), "{:?}", report.set_aside);
    assert_eq!(played.first_difference(&loaded), None, "{save}");
    assert_eq!(parent(&loaded, "created/1/light"), Some("yard".to_owned()));
}

#[test]
fn a_run_carried_through_a_save_into_a_new_start_is_the_run_saved_and_loaded_into_it() {
    let mut scenes = yard_lamp_and_village();
    let mut played = run(&mut scenes);
    let lamp = scenes.resolve(played.registry(), LAMP).expect("resolved");
    played.spawn_placement(&lamp, None).expect("placed");
    played.insert(entity(&played, "yard/shed"), Hidden);
    // The lamp changes on disk: the game makes a new start world from it.
    scenes.insert_file(
        LAMP,
        "lamp",
        LAMP_FILE.replace("intensity = 40.0", "intensity = 60.0"),
    );
    let (carried, report) = played
        .carry(yard(&mut scenes), &mut scenes, &mut |_, _| {})
        .expect("carried");
    assert!(report.set_aside.is_empty(), "{:?}", report.set_aside);
    let save = played.save(&Default::default()).expect("saved");
    let mut loaded = yard(&mut scenes);
    loaded
        .load(&save, &mut scenes, &mut |_, _| {})
        .expect("loaded");
    assert_eq!(carried.first_difference(&loaded), None);
    // The lamp changed in every placement but where an override or the run changed it.
    for (id, expected) in [
        ("yard/porch/light", 60.0),
        ("yard/shed/light", 60.0),
        ("village/well/light", 60.0),
        ("created/3/light", 60.0),
        ("yard/gate/light", 10.0),
        ("created/1/light", 5.0),
    ] {
        assert_eq!(intensity(&carried, id), expected, "{id}");
    }
    assert!(value(&carried, "yard/shed", "Hidden").is_some());
    assert!(value(&carried, "village/square/post", "Hidden").is_some());
}

#[test]
fn saves_of_what_a_run_spawned_cut_or_changed_at_every_character_are_refused_or_loaded() {
    let mut scenes = yard_lamp_and_village();
    let save = run(&mut scenes).save(&Default::default()).expect("saved");
    for (at, _) in save.char_indices() {
        let changed = format!("{}#{}", &save[..at], &save[at + 1..]);
        for text in [&save[..at], &changed] {
            let mut loaded = yard(&mut scenes);
            if loaded.load(text, &mut scenes, &mut |_, _| {}).is_err() {
                assert_eq!(loaded.first_difference(&yard(&mut scenes)), None, "{text}");
                assert_eq!(loaded.len(), yard(&mut scenes).len(), "{text}");
            }
        }
    }
}

#[test]
fn a_save_that_spawns_under_an_identity_in_use_is_refused_or_set_aside() {
    let mut scenes = yard_lamp_and_village();
    let save = run(&mut scenes).save(&Default::default()).expect("saved");
    // A start that created one more entity outside scenes: what the run created takes the numbers
    // that follow the world's count.
    let mut more = yard(&mut scenes);
    let extra = more.spawn();
    more.load(&save, &mut scenes, &mut |_, _| {})
        .expect("loaded");
    assert_eq!(
        more.persistent_id(extra),
        Some("created/1".parse().unwrap())
    );
    assert!(value(&more, "created/2/light", "test::Glow").is_some());
    // A save that does not say what its start created, into such a world: refused, unchanged.
    let edited = save.replace("start = { created = 1 }\n", "");
    assert_ne!(edited, save);
    let mut taken = yard(&mut scenes);
    taken.spawn();
    let before = taken.len();
    assert!(matches!(
        taken.load(&edited, &mut scenes, &mut |_, _| {}),
        Err(slime_world::DocumentError::IdentityInUse(_))
    ));
    assert_eq!(taken.len(), before);
    assert!(taken.find(&"village".parse().unwrap()).is_none());
    // A start world that spawns the village itself: its entities take the run's changes, the scene
    // kept.
    let mut spawning = yard(&mut scenes);
    spawn(&mut spawning, &mut scenes, VILLAGE, "village");
    let kept = entity(&spawning, "village");
    let report = spawning
        .load(&save, &mut scenes, &mut |_, _| {})
        .expect("loaded");
    assert!(report.set_aside.is_empty(), "{:?}", report.set_aside);
    assert_eq!(entity(&spawning, "village"), kept);
    assert!(value(&spawning, "village/square/post", "Hidden").is_some());
}

#[test]
fn a_run_that_spawns_another_scene_under_a_key_of_its_start_saves_and_loads_it() {
    let mut scenes = yard_lamp_and_village();
    let start = |scenes: &mut SceneLibrary| {
        let mut world = yard(scenes);
        spawn(&mut world, scenes, LAMP, "level");
        world.settle();
        world
    };
    // The run changes level: the lamp under `level` gives way to the village.
    let mut played = start(&mut scenes);
    played.despawn(entity(&played, "level"));
    let village = scenes
        .resolve(played.registry(), VILLAGE)
        .expect("resolved");
    played.spawn_scene("level", &village).expect("spawned");
    played.insert(entity(&played, "level/square/post"), Hidden);
    let save = played.save(&Default::default()).expect("saved");
    assert!(
        save.contains(&format!("level = {{ scene = \"{VILLAGE}\"")),
        "{save}"
    );
    let mut loaded = start(&mut scenes);
    let report = loaded
        .load(&save, &mut scenes, &mut |_, _| {})
        .expect("loaded");
    assert!(report.set_aside.is_empty(), "{:?}", report.set_aside);
    assert_eq!(played.first_difference(&loaded), None, "{save}");
    assert_eq!(loaded.save(&Default::default()).expect("saved again"), save);
}

#[test]
fn a_reference_to_an_entity_of_what_a_run_spawned_destroyed_since_saves_the_same_bytes_again() {
    let mut scenes = yard_lamp_and_village();
    let mut played = yard(&mut scenes);
    let lamp = scenes.resolve(played.registry(), LAMP).expect("resolved");
    played.spawn_placement(&lamp, None).expect("placed");
    // The light, which aims at the post, moved out, then the post destroyed.
    played
        .set_parent(entity(&played, "created/1/light"), None)
        .expect("alive");
    played.despawn(entity(&played, "created/1/post"));
    let save = played.save(&Default::default()).expect("saved");
    let mut loaded = yard(&mut scenes);
    let report = loaded
        .load(&save, &mut scenes, &mut |_, _| {})
        .expect("loaded");
    assert!(report.set_aside.is_empty(), "{:?}", report.set_aside);
    assert_eq!(played.first_difference(&loaded), None, "{save}");
    assert_eq!(loaded.save(&Default::default()).expect("saved again"), save);
}

#[test]
fn what_a_run_spawned_under_an_entity_marked_unsaved_is_left_out_whatever_the_order_of_the_file() {
    // The child before its parent, which holds Unsaved.
    const ODD: AssetId = AssetId::from_u128(10);
    let mut scenes = yard_lamp_and_village();
    scenes.insert_file(
        ODD,
        "odd",
        "[entities.a]\nparent = \"z\"\n\n[entities.z.components]\nUnsaved = {}\n",
    );
    let mut played = yard(&mut scenes);
    spawn(&mut played, &mut scenes, ODD, "odd");
    played.despawn(entity(&played, "odd/a"));
    let save = played.save(&Default::default()).expect("saved");
    assert!(!save.contains("odd/a"), "{save}");
}

#[test]
fn siblings_keep_their_order_whatever_their_records_hold() {
    let mut scenes = library(&[(YARD, "yard", YARD_FILE), (LAMP, "lamp", LAMP_FILE)]);
    let mut edited = world();
    let root = spawn(&mut edited, &mut scenes, YARD, "yard");
    // An entity created first, whose record holds its components, then the placements, whose
    // records are small.
    let hook = edited.spawn();
    edited.insert(hook, Name("hook".to_owned()));
    edited.set_parent(hook, Some(root)).expect("alive");
    for id in ["yard/porch", "yard/gate", "yard/shed"] {
        let placed = entity(&edited, id);
        edited.set_parent(placed, None).expect("alive");
        edited.set_parent(placed, Some(root)).expect("alive");
    }
    let (written, reread) = written_back(&mut edited, &mut scenes);
    let children = children_of(&reread, "yard");
    assert!(
        children[0].starts_with("yard/hook-"),
        "{children:?}\n{}",
        written.text
    );
    assert_eq!(children[1..], ["yard/porch", "yard/gate", "yard/shed"]);
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
fn what_resolving_spawning_writing_and_saving_scenes_costs() {
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
    for count in [10_000, 80_000] {
        let mut world = world();
        let root = world.spawn();
        for index in 0..count {
            let entity = world.spawn();
            world.insert(entity, Name(format!("e{index}")));
            world.set_parent(entity, Some(root)).expect("alive");
        }
        let started = std::time::Instant::now();
        let first = world.write_scene(root).expect("written");
        let gave = started.elapsed();
        let started = std::time::Instant::now();
        world.write_scene(root).expect("written");
        eprintln!(
            "{count} entities created outside scenes: written in {gave:?}, then again in {:?}, {} bytes",
            started.elapsed(),
            first.text.len()
        );
    }
    // Placements made while a world runs, each moved and a value of it changed, saved and loaded.
    for entities in [10, 100] {
        let mut scenes = library(&[(LAMP, "lamp", &chain_prefab(entities))]);
        let mut world = world();
        let lamp = scenes.resolve(world.registry(), LAMP).expect("resolved");
        for index in 0..1_000 {
            let placed = world.spawn_placement(&lamp, None).expect("placed");
            world.insert(
                placed,
                Transform::from_translation(glam::DVec3::new(f64::from(index), 0.0, 0.0)),
            );
            let first = entity(&world, &format!("created/{}/e0", index + 1));
            world
                .set_property(
                    first,
                    "test::Glow",
                    &"intensity".parse().unwrap(),
                    Value::F32(2.0),
                )
                .expect("in range");
        }
        let started = std::time::Instant::now();
        let save = world.save(&Default::default()).expect("saved");
        let saved = started.elapsed();
        let started = std::time::Instant::now();
        let mut loaded = self::world();
        loaded
            .load(&save, &mut scenes, &mut |_, _| {})
            .expect("loaded");
        eprintln!(
            "1000 placements of {entities} entities made while the world runs: {} entities, saved in {saved:?}, loaded in {:?}, {} bytes",
            world.len(),
            started.elapsed(),
            save.len()
        );
        assert_eq!(loaded.len(), world.len());
    }
}
