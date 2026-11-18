//! Saves of the earlier layouts that the reader reads: the fixture of layout 5, frozen as the
//! writer of layout 5 wrote it, loaded as it is, and through the reader of later layouts made for
//! the test alone (`docs/adr/0288-saves-migrated-from-their-layout-5-on.md`).

use std::collections::BTreeMap;

use glam::DVec3;
use slime_assets::AssetId;
use slime_reflect::{Reflect, Value};

use super::{DocumentError, EARLIEST_LAYOUT, LAYOUT_UPGRADES, LAYOUT_VERSION, LayoutUpgrade};
use crate::{Entity, Name, SceneLibrary, Transform, World};

/// The fixture of layout 5: what [`played`] saves, written by the engine's writer of layout 5 on
/// 2026-11-18, and never written again. It holds every section of the layout: what the game says
/// of the save, the counts of the entities created, the scenes under their keys, a placement made
/// while running, entities and components removed, a parent changed, an entity created outside
/// scenes, components added and changed, what the world keeps of a scene unloaded, an entity
/// adopted, and the version of a component's type past 1.
const LAYOUT_5: &str = include_str!("../tests/reference/saves/layout-5.toml");

/// The camp, which stays where the run goes: a fire, a log, a tent and a stump.
const CAMP: AssetId = AssetId::from_u128(51);
/// The town, the level where the run starts: a chest that keeps what the run did to it, and a
/// companion.
const TOWN: AssetId = AssetId::from_u128(52);
/// The cave, the level where the run goes from the town: a rock.
const CAVE: AssetId = AssetId::from_u128(53);
/// A torch, a prefab that the run places: a flame.
const TORCH: AssetId = AssetId::from_u128(54);

/// What a lamp gives, at its type's version 2: its `power` was called `watts` at version 1.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Lamp", version = 2, upgrade = upgrade_lamp)]
struct Lamp {
    power: f32,
}
impl crate::Component for Lamp {}

fn upgrade_lamp(version: u32, value: Value, _partial: bool) -> Result<Value, String> {
    match (version, value) {
        (1, Value::Struct(fields)) => Ok(Value::Struct(
            fields
                .into_iter()
                .map(|(name, value)| match name.as_str() {
                    "watts" => ("power".to_owned(), value),
                    _ => (name, value),
                })
                .collect(),
        )),
        (version, _) => Err(format!("no lamp of version {version} was a table")),
    }
}

/// What an entity looks at.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Gaze")]
struct Gaze {
    at: Entity,
}
impl crate::Component for Gaze {}

/// The scenes of the fixture's run.
fn scenes() -> SceneLibrary {
    let mut scenes = SceneLibrary::new();
    let entities = |names: &[&str]| {
        names
            .iter()
            .map(|name| format!("[entities.{name}.components]\nName = \"{name}\"\n\n"))
            .collect::<String>()
    };
    scenes.insert_file(
        CAMP,
        "camp",
        format!(
            "{}[entities.log.components]\nName = \"log\"\n\
             Transform = {{ translation = {{ x = 1.0, y = 0.0, z = 0.0 }} }}\n",
            entities(&["fire", "tent", "stump"])
        ),
    );
    scenes.insert_file(
        TOWN,
        "town",
        format!(
            "{}[entities.chest.components]\nName = \"chest\"\nPersistence = \"Persistent\"\n",
            entities(&["companion"])
        ),
    );
    scenes.insert_file(CAVE, "cave", entities(&["rock"]));
    scenes.insert_file(TORCH, "torch", entities(&["flame"]));
    scenes
}

/// The entity whose persistent identity `id` writes.
fn entity(world: &World, id: &str) -> Entity {
    world
        .find(&id.parse().expect("an identity"))
        .unwrap_or_else(|| panic!("no `{id}`"))
}

/// The start of the fixture's run: the camp under `camp`, the town under `level`, and the hero,
/// created outside scenes under a key.
fn start(scenes: &mut SceneLibrary) -> World {
    let mut world = World::new();
    world.register::<Lamp>();
    world.register::<Gaze>();
    for (key, id) in [("camp", CAMP), ("level", TOWN)] {
        let scene = scenes.resolve(world.registry(), id).expect("resolved");
        world.spawn_scene(key, &scene).expect("spawned");
    }
    let hero = world.spawn_keyed("hero").expect("a free key");
    world.insert(hero, Name("hero".to_owned()));
    world.settle();
    world
}

/// The fixture's run: in the camp, the fire lit, the log charred and carried, the tent's name
/// lost, the stump gone; the hero looking at the fire, carrying a pack; a torch placed by the fire;
/// in the town, the chest moved, then the companion following the hero to the cave.
fn played(scenes: &mut SceneLibrary) -> World {
    let mut run = start(scenes);
    let hero = entity(&run, "hero");
    let fire = entity(&run, "camp/fire");
    run.insert(fire, Lamp { power: 60.0 });
    let log = entity(&run, "camp/log");
    run.insert(log, Name("charred log".to_owned()));
    run.insert(log, Transform::from_translation(DVec3::new(1.0, 0.5, 0.0)));
    run.set_parent(log, Some(hero)).expect("alive");
    run.remove::<Name>(entity(&run, "camp/tent"));
    run.despawn(entity(&run, "camp/stump"));
    run.insert(hero, Gaze { at: fire });
    let pack = run.spawn();
    run.insert(pack, Name("pack".to_owned()));
    run.set_parent(pack, Some(hero)).expect("alive");
    let torch = scenes.resolve(run.registry(), TORCH).expect("resolved");
    run.spawn_placement(&torch, Some(fire)).expect("placed");
    let chest = entity(&run, "level/chest");
    run.insert(
        chest,
        Transform::from_translation(DVec3::new(0.0, 1.0, 0.0)),
    );
    let companion = entity(&run, "level/companion");
    run.adopt(companion, "companion").expect("adopted");
    run.set_parent(companion, Some(hero)).expect("alive");
    let cave = scenes.resolve(run.registry(), CAVE).expect("resolved");
    run.spawn_scene("level", &cave)
        .expect("the town unloaded first");
    run.settle();
    run
}

/// What the game says of the fixture.
fn about() -> BTreeMap<String, String> {
    BTreeMap::from([("made on".to_owned(), "the fixture's run".to_owned())])
}

/// `save` loaded into the fixture's start by a reader whose layouts `upgrades` give, and what
/// loading it left aside.
fn loaded(save: &str, upgrades: &[LayoutUpgrade]) -> Result<World, DocumentError> {
    let mut scenes = scenes();
    let mut world = start(&mut scenes);
    let loaded = world.load_through(save, &mut scenes, &mut |_, _| {}, upgrades)?;
    assert!(
        loaded.set_aside.is_empty() && loaded.scenes_changed.is_empty(),
        "{loaded:?}"
    );
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    Ok(world)
}

#[test]
fn the_fixture_of_layout_5_loads_as_the_run_that_wrote_it() {
    let played = played(&mut scenes());
    let world = loaded(LAYOUT_5, LAYOUT_UPGRADES).expect("a save of layout 5");
    assert_eq!(played.first_difference(&world), None);
    assert_eq!(
        world.save(&about()).expect("saved"),
        played.save(&about()).expect("saved"),
        "saved again as the run saves itself"
    );
    assert_eq!(World::about(LAYOUT_5).expect("a save"), about());
}

/// Brings a save to the next layout of the test's, one past the layouts that the reader before it
/// reads: notes in what the game says of it the layouts that it went from and to, after what is
/// noted there already, and changes nothing else, nothing then that a reader of the engine's latest
/// layout reads but what it is told.
fn noted(mut fields: Vec<(String, Value)>) -> Result<Vec<(String, Value)>, String> {
    let layout = match fields.iter().find(|(name, _)| name == "layout") {
        Some((_, Value::Int(layout))) => *layout,
        given => return Err(format!("given no layout, but {given:?}")),
    };
    let note = format!("{layout} to {}", layout + 1);
    let at = match fields.iter().position(|(name, _)| name == "about") {
        Some(at) => at,
        None => {
            fields.push(("about".to_owned(), Value::Struct(Vec::new())));
            fields.len() - 1
        }
    };
    let Value::Struct(about) = &mut fields[at].1 else {
        return Err("what the game says of it is not a table".to_owned());
    };
    match about.iter_mut().find(|(name, _)| name == "upgraded") {
        Some((_, Value::Text(notes))) => *notes = format!("{notes}, {note}"),
        _ => about.push(("upgraded".to_owned(), Value::Text(note))),
    }
    Ok(fields)
}

/// The upgrades of a reader whose layouts reach `more` past the engine's latest: the engine's,
/// then the test's.
fn upgrades(more: usize) -> Vec<LayoutUpgrade> {
    let mut upgrades = LAYOUT_UPGRADES.to_vec();
    upgrades.extend(std::iter::repeat_n(noted as LayoutUpgrade, more));
    upgrades
}

/// What `about`, the fixture's, says once upgraded as `upgraded` says.
fn about_upgraded(upgraded: &str) -> BTreeMap<String, String> {
    let mut about = about();
    about.insert("upgraded".to_owned(), upgraded.to_owned());
    about
}

/// `save`, of the engine's latest layout, said to be of `layout`.
fn said_of(save: &str, layout: u64) -> String {
    let line = format!("layout = {LAYOUT_VERSION}\n");
    assert!(save.contains(&line), "{save}");
    save.replacen(&line, &format!("layout = {layout}\n"), 1)
}

#[test]
fn the_fixture_of_layout_5_reads_through_the_reader_of_a_later_layout_each_upgrade_in_turn() {
    let played = played(&mut scenes());
    let latest = LAYOUT_VERSION;
    for (more, upgraded) in [
        (1, format!("{latest} to {}", latest + 1)),
        (
            2,
            format!(
                "{latest} to {}, {} to {}",
                latest + 1,
                latest + 1,
                latest + 2
            ),
        ),
    ] {
        let upgrades = upgrades(more);
        let world = loaded(LAYOUT_5, &upgrades).expect("a save of layout 5");
        assert_eq!(played.first_difference(&world), None, "{upgraded}");
        assert_eq!(
            World::about_through(LAYOUT_5, &upgrades).expect("a save"),
            about_upgraded(&upgraded)
        );
    }
    // A save of the test's first layout, read by the reader of its second, takes the last upgrade
    // alone; one of its second, none. Both hold what the engine's latest writes, the test's
    // layouts changing nothing of it.
    let upgrades = upgrades(2);
    let save = played.save(&about()).expect("saved");
    for (layout, upgraded) in [
        (
            latest + 1,
            Some(format!("{} to {}", latest + 1, latest + 2)),
        ),
        (latest + 2, None),
    ] {
        let save = said_of(&save, layout);
        let world = loaded(&save, &upgrades).expect("a save that the reader reads");
        assert_eq!(played.first_difference(&world), None);
        let about = World::about_through(&save, &upgrades).expect("a save");
        assert_eq!(
            about,
            upgraded.map_or_else(self::about, |upgraded| about_upgraded(&upgraded))
        );
    }
}

#[test]
fn a_save_of_a_layout_that_the_reader_does_not_read_is_refused_naming_the_layouts_it_reads() {
    let upgrades = upgrades(2);
    let latest = LAYOUT_VERSION + 2;
    let save = played(&mut scenes()).save(&about()).expect("saved");
    for (layout, wrote) in [
        (
            EARLIEST_LAYOUT - 1,
            "an earlier version of the engine wrote",
        ),
        (latest + 1, "a later version of the engine wrote"),
    ] {
        let save = said_of(&save, layout);
        let refused = loaded(&save, &upgrades).expect_err("refused");
        assert!(
            matches!(refused, DocumentError::Layout { found, read } if found == layout && read == latest),
            "{refused:?}"
        );
        let message = refused.to_string();
        let reads = format!("this program reads saves of layouts {EARLIEST_LAYOUT} to {latest}");
        assert!(
            message.contains(wrote) && message.ends_with(&reads),
            "{message}"
        );
        assert!(matches!(
            World::about_through(&save, &upgrades),
            Err(DocumentError::Layout { .. })
        ));
    }
    // The engine's own reader refuses the layout after its own.
    let refused =
        loaded(&said_of(&save, LAYOUT_VERSION + 1), LAYOUT_UPGRADES).expect_err("refused");
    assert!(
        matches!(
            refused,
            DocumentError::Layout {
                read: LAYOUT_VERSION,
                ..
            }
        ),
        "{refused:?}"
    );
}

#[test]
fn a_save_that_an_upgrade_of_its_layout_refuses_is_refused_whole_naming_both_layouts() {
    fn refuses(_: Vec<(String, Value)>) -> Result<Vec<(String, Value)>, String> {
        Err("its chests are not tables".to_owned())
    }
    let mut upgrades = upgrades(1);
    upgrades.push(refuses);
    let mut scenes = scenes();
    let mut world = start(&mut scenes);
    let before = start(&mut scenes);
    let refused = world
        .load_through(LAYOUT_5, &mut scenes, &mut |_, _| {}, &upgrades)
        .expect_err("refused");
    let DocumentError::Read(error) = &refused else {
        panic!("{refused:?}");
    };
    let from = LAYOUT_VERSION + 1;
    assert_eq!(
        error.problem().to_string(),
        format!(
            "a save of layout {from} that cannot be brought to layout {}: its chests are not \
             tables",
            from + 1
        )
    );
    assert_eq!(
        error.line_and_column(),
        None,
        "an upgraded save has no lines"
    );
    assert_eq!(before.first_difference(&world), None, "the world unchanged");
}
