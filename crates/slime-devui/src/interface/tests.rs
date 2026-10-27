//! The world panel and the editors of reflected values on the engine's interface, without a GPU:
//! built for every kind of value, used through the interface as a person or an agent uses it,
//! each edit read as a command or set in a value outside a world.

use slime_color::LinearColor;
use slime_input::{ButtonState, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
use slime_reflect::{Path, Reflect, Value};
use slime_ui::look::{self, Colors, Density};
use slime_ui::style::{FlexDirection, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{Element, ElementId, LayoutDirection, Meaning, Role, RootSettings, Ui};
use slime_world::glam::{DVec3, Quat, Vec3};
use slime_world::{Command, Entity, Light, World};

use super::{ValueEditor, WorldPanel};
use crate::world_panel::tests::world;
use crate::world_panel::{angles, rotation};

/// An interface in the editor's look, of one root of 420 by 1600 logical pixels at scale 1.
fn interface() -> (Ui, ElementId) {
    let mut fonts = Fonts::new();
    look::register_fonts(&mut fonts);
    let mut ui = Ui::new(fonts);
    ui.set_style(
        &[look::style_sheet()],
        &look::themes(Colors::Dark, Density::Comfortable),
    )
    .expect("the look's documents are valid");
    let settings = RootSettings {
        size: [420.0, 1600.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let mut column = Style::default();
    column.layout.direction = FlexDirection::Column;
    let root = ui.add_root(settings, column);
    (ui, root)
}

/// The world panel over the tests' world, in an interface.
struct Bench {
    ui: Ui,
    root: ElementId,
    panel: WorldPanel,
    world: World,
    entities: Vec<Entity>,
}

impl Bench {
    fn new() -> Self {
        let (ui, root) = interface();
        let (world, entities) = world();
        let mut bench = Self {
            ui,
            root,
            panel: WorldPanel::default(),
            world,
            entities,
        };
        assert!(bench.frame().is_empty());
        bench
    }

    /// A frame of a program: the actions that the input queued, read by the panel, then the panel
    /// built and the interface laid out. Gives the panel's commands, which it applies to the world,
    /// and the actions that were not the panel's.
    fn frame(&mut self) -> Vec<Command> {
        let mut actions = self.ui.take_actions();
        let commands = self.panel.act(&mut actions, &self.world);
        assert!(
            actions.is_empty(),
            "every action is the panel's: {actions:?}"
        );
        for command in &commands {
            if let Err(error) = self.world.apply(command) {
                self.panel.refused(&error);
            }
        }
        self.panel.build(&mut self.ui, self.root, &self.world);
        self.ui.frame();
        commands
    }

    /// The element named `name`, of `role`.
    fn find(&self, role: Role, name: &str) -> Meaning {
        let found: Vec<Meaning> = self
            .ui
            .meaning()
            .into_iter()
            .filter(|meaning| meaning.role == role && meaning.name == name)
            .collect();
        assert_eq!(found.len(), 1, "one {role:?} named {name:?}: {found:?}");
        found.into_iter().next().expect("found")
    }

    /// A press and a release of the pointer in the middle of `id`.
    fn click(&mut self, id: ElementId) {
        let rect = self.ui.rect(id).expect("laid out");
        self.ui
            .pointer_moved(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
        self.ui
            .pointer_button(MouseButton::Left, ButtonState::Pressed);
        self.ui
            .pointer_button(MouseButton::Left, ButtonState::Released);
    }
}

fn named(key: NamedKey) -> KeyEvent {
    KeyEvent {
        physical: KeyCode::Unidentified,
        logical: Key::Named(key),
        text: None,
        state: ButtonState::Pressed,
        repeat: false,
    }
}

/// The command that sets the part at `path`, as text, of `component` of `entity` to `value`.
fn set(world: &World, entity: Entity, component: &str, path: &str, value: Value) -> Command {
    Command::SetProperty {
        entity: world.persistent_id(entity).expect("an identity"),
        component: component.to_owned(),
        path: path.parse::<Path>().expect("a path"),
        value,
    }
}

#[test]
fn every_component_is_built_and_nothing_is_touched() {
    let mut bench = Bench::new();
    let none = bench.find(Role::Label, "Select an entity to see its components.");
    assert!(none.bounds.is_some());
    for entity in bench.entities.clone() {
        bench.panel.select(entity);
        for _ in 0..3 {
            assert_eq!(bench.frame(), [], "nothing is touched");
        }
        for component in bench.world.component_names(entity) {
            let section = bench.find(Role::Details, component);
            assert!(section.state.expanded, "{component} opens at first");
        }
    }
}

#[test]
fn a_field_crossed_or_given_the_focus_writes_nothing() {
    let mut bench = Bench::new();
    let everything = bench.entities[4];
    bench.panel.select(everything);
    bench.frame();
    // Every field of the component crossed with Tab, as G33 found egui's writing its rounding.
    let ratio = bench.find(Role::SpinButton, "ratio").id;
    bench.ui.set_focus(Some(ratio));
    for _ in 0..40 {
        bench.ui.key(&named(NamedKey::Tab));
    }
    bench.ui.set_focus(None);
    assert_eq!(bench.frame(), []);
}

#[test]
fn numbers_booleans_and_text_are_set_as_the_user_commits_them() {
    let mut bench = Bench::new();
    let everything = bench.entities[4];
    bench.panel.select(everything);
    bench.frame();
    let ratio = bench.find(Role::SpinButton, "ratio").id;
    assert!(bench.ui.set_value(ratio, "0.25 + 0.5"));
    let count = bench.find(Role::SpinButton, "count").id;
    assert!(bench.ui.set_value(count, "4.6"));
    let label = bench.find(Role::TextInput, "label").id;
    assert!(bench.ui.set_value(label, "renamed"));
    let on = bench.find(Role::CheckBox, "on").id;
    bench.click(on);
    let world = &bench.world;
    let expected = [
        set(
            world,
            everything,
            "test::Everything",
            "ratio",
            Value::F64(0.75),
        ),
        set(
            world,
            everything,
            "test::Everything",
            "count",
            Value::Int(5),
        ),
        set(
            world,
            everything,
            "test::Everything",
            "label",
            Value::Text("renamed".to_owned()),
        ),
        set(
            world,
            everything,
            "test::Everything",
            "on",
            Value::Bool(false),
        ),
    ];
    assert_eq!(bench.frame(), expected);
    // The world took them, and the fields keep what the user typed.
    assert_eq!(bench.frame(), []);
    assert_eq!(bench.ui.field_text(ratio), Some("0.75"));
}

#[test]
fn a_number_in_an_option_sets_the_option_and_its_box_empties_it() {
    let mut bench = Bench::new();
    let everything = bench.entities[4];
    bench.panel.select(everything);
    bench.frame();
    // A field of meters reads centimeters, and keeps the `f32`'s shortest text.
    let reach = bench.find(Role::SpinButton, "reach").id;
    assert!(bench.ui.set_value(reach, "40cm"));
    let world = &bench.world;
    let held = Value::Option(Some(Box::new(Value::F32(0.4))));
    let expected = set(world, everything, "test::Everything", "reach", held);
    assert_eq!(bench.frame(), [expected]);
    bench.frame();
    assert_eq!(bench.ui.field_text(reach), Some("0.4"));
    let holds = bench.find(Role::CheckBox, "reach holds a value").id;
    bench.click(holds);
    let world = &bench.world;
    let expected = set(
        world,
        everything,
        "test::Everything",
        "reach",
        Value::Option(None),
    );
    assert_eq!(bench.frame(), [expected]);
}

#[test]
fn vectors_rotations_and_colors_are_set_whole() {
    let mut bench = Bench::new();
    let everything = bench.entities[4];
    bench.panel.select(everything);
    bench.frame();
    let offset_y = bench.find(Role::SpinButton, "offset y").id;
    assert!(bench.ui.set_value(offset_y, "2"));
    let axis_z = bench.find(Role::SpinButton, "axis z").id;
    assert!(bench.ui.set_value(axis_z, "0.5"));
    let tint_g = bench.find(Role::SpinButton, "tint g").id;
    assert!(bench.ui.set_value(tint_g, "0.5"));
    let world = &bench.world;
    let tint = LinearColor {
        r: 1.0,
        g: 0.5,
        b: 1.0,
        a: 1.0,
    };
    let expected = [
        set(
            world,
            everything,
            "test::Everything",
            "offset",
            DVec3::new(1.0, 2.0, 0.0).to_value(),
        ),
        set(
            world,
            everything,
            "test::Everything",
            "axis",
            Vec3::new(0.0, 1.0, 0.5).to_value(),
        ),
        set(
            world,
            everything,
            "test::Everything",
            "tint",
            Value::Option(Some(Box::new(tint.to_value()))),
        ),
    ];
    assert_eq!(bench.frame(), expected);
    // A rotation as three angles, about Y, X then Z: the angle typed about X is kept within 90°,
    // and the angles typed are kept while the rotation is the one they made.
    let turn_x = bench.find(Role::SpinButton, "turn x").id;
    assert!(bench.ui.set_value(turn_x, "120"));
    let commands = bench.frame();
    let [Command::SetProperty { value, .. }] = commands.as_slice() else {
        panic!("one rotation set: {commands:?}");
    };
    let turned = Quat::from_value(value).expect("a rotation");
    let expected = rotation([0.5f32.to_degrees(), 90.0, 0.0]);
    assert!(
        turned.angle_between(expected) < 1e-4,
        "{:?}",
        angles(turned)
    );
    bench.frame();
    let turn_y = bench.find(Role::SpinButton, "turn y").id;
    assert_eq!(bench.ui.field_text(turn_y), Some("28.64789"));
}

#[test]
fn an_enum_takes_the_variants_it_can_start_in() {
    let mut bench = Bench::new();
    let point = bench.entities[2];
    bench.panel.select(point);
    bench.frame();
    let light = bench.find(Role::ComboBox, "Light").id;
    assert_eq!(
        bench
            .ui
            .meaning_of(light)
            .and_then(|meaning| meaning.value)
            .as_deref(),
        Some("Point")
    );
    assert!(bench.ui.set_value(light, "Directional"));
    let commands = bench.frame();
    assert_eq!(commands.len(), 1, "{commands:?}");
    bench.frame();
    assert!(matches!(
        bench.world.get::<Light>(point),
        Some(Light::Directional { .. })
    ));
    // A spot light's cones start at one angle, which its check refuses: the world says why.
    let light = bench.find(Role::ComboBox, "Light").id;
    assert!(bench.ui.set_value(light, "Spot"));
    bench.frame();
    bench.frame();
    let refusal = bench
        .ui
        .meaning()
        .into_iter()
        .find(|meaning| meaning.name.contains("cones"));
    assert!(refusal.is_some(), "the refusal shows");
    // A variant that holds a reference cannot start: disabled, it cannot be chosen.
    let everything = bench.entities[4];
    bench.panel.select(everything);
    bench.frame();
    let mode = bench.find(Role::ComboBox, "mode").id;
    assert!(bench.ui.set_value(mode, "Off"));
    bench.frame();
    bench.frame();
    let mode = bench.find(Role::ComboBox, "mode").id;
    assert!(!bench.ui.set_value(mode, "Aimed"), "no entity to aim at");
}

#[test]
fn a_reference_and_a_row_select_their_entity_and_the_filter_keeps_names() {
    let mut bench = Bench::new();
    let everything = bench.entities[4];
    bench.panel.select(everything);
    bench.frame();
    let door = bench.entities[0];
    let target = bench
        .ui
        .meaning()
        .into_iter()
        .find(|meaning| meaning.name == "door" && meaning.path.contains("field target"))
        .expect("the reference to the door")
        .id;
    bench.click(target);
    bench.frame();
    assert_eq!(bench.panel.selected(), Some(door));
    bench.frame();
    // The rows: one per entity, the door's selected.
    let rows: Vec<Meaning> = bench
        .ui
        .meaning()
        .into_iter()
        .filter(|meaning| meaning.role == Role::Button && meaning.path.contains("/list/"))
        .collect();
    assert_eq!(rows.len(), bench.entities.len());
    assert!(
        rows.iter()
            .any(|row| row.name == "door" && row.state.selected)
    );
    let filter = bench.find(Role::SearchInput, "filter by name").id;
    assert!(bench.ui.set_value(filter, "DOO"));
    bench.frame();
    bench.frame();
    let rows: Vec<String> = bench
        .ui
        .meaning()
        .into_iter()
        .filter(|meaning| meaning.role == Role::Button && meaning.path.contains("/list/"))
        .map(|meaning| meaning.name)
        .collect();
    assert_eq!(rows, ["door"]);
    // A press on a row selects its entity.
    assert!(bench.ui.set_value(filter, ""));
    bench.frame();
    bench.frame();
    let spot = bench.entities[3];
    let row = format!("{spot:?}");
    let row = bench.find(Role::Button, &row).id;
    bench.click(row);
    bench.frame();
    assert_eq!(bench.panel.selected(), Some(spot));
}

#[test]
fn a_list_of_many_entities_builds_only_the_rows_that_show() {
    let (mut ui, root) = interface();
    let mut world = World::new();
    for _ in 0..100_000 {
        world.spawn();
    }
    let mut panel = WorldPanel::default();
    for _ in 0..2 {
        panel.build(&mut ui, root, &world);
        ui.frame();
    }
    let rows = ui
        .meaning()
        .into_iter()
        .filter(|meaning| meaning.path.contains("/list/"))
        .count();
    // A list 160 pixels high, of rows 22 high, shows eight of them, and builds no others.
    assert!((1..=10).contains(&rows), "{rows} rows built");
}

/// Settings outside a world, as a view's.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Knobs")]
struct Knobs {
    /// How large.
    #[reflect(range(min = 0.25, max = 2.0))]
    scale: f32,
    /// Whether it is lit.
    lit: bool,
}

#[test]
fn a_value_outside_a_world_takes_the_edits_that_its_ranges_hold() {
    let (mut ui, root) = interface();
    let mut slot = None;
    ui.build(root, 0, |children| {
        slot = Some(children.add_slot("knobs", Element::container(Style::default())));
    });
    let slot = slot.expect("built");
    let mut knobs = Knobs {
        scale: 1.0,
        lit: false,
    };
    let mut editor = ValueEditor::default();
    editor.build(&mut ui, slot, "knobs", &knobs);
    ui.frame();
    let find = |ui: &Ui, name: &str| {
        ui.meaning()
            .into_iter()
            .find(|meaning| meaning.name == name && meaning.role != Role::Label)
            .expect("found")
            .id
    };
    assert!(ui.set_value(find(&ui, "scale"), "5"));
    let actions = ui.take_actions();
    assert!(!editor.apply(&actions, &mut knobs), "out of its range");
    assert!(ui.set_value(find(&ui, "scale"), "0.5"));
    let rect = ui.rect(find(&ui, "lit")).expect("laid out");
    ui.pointer_moved(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    let actions = ui.take_actions();
    assert!(editor.apply(&actions, &mut knobs));
    assert_eq!(
        knobs,
        Knobs {
            scale: 0.5,
            lit: true
        }
    );
    // Built again for the value changed, the field keeps what was committed.
    editor.build(&mut ui, slot, "knobs", &knobs);
    ui.frame();
    assert_eq!(ui.field_text(find(&ui, "scale")), Some("0.5"));
}

#[test]
fn an_angle_in_radians_is_shown_and_typed_in_degrees() {
    let mut bench = Bench::new();
    let spot = bench.entities[3];
    bench.panel.select(spot);
    bench.frame();
    let inner = bench.find(Role::SpinButton, "inner_cone_angle").id;
    assert_eq!(bench.ui.field_text(inner), Some("11.459156"));
    assert!(bench.ui.set_value(inner, "10"));
    let world = &bench.world;
    let radians = Value::F32(10f32.to_radians());
    let expected = set(world, spot, "Light", "inner_cone_angle", radians);
    assert_eq!(bench.frame(), [expected]);
    // A rotation of none shows no `-0`.
    let door = bench.entities[0];
    bench.panel.select(door);
    bench.frame();
    for axis in ["x", "y", "z"] {
        let angle = bench.find(Role::SpinButton, &format!("rotation {axis}")).id;
        assert_eq!(bench.ui.field_text(angle), Some("0"), "about {axis}");
    }
}

/// The names that `ui` means, of every element drawn.
fn names(ui: &Ui) -> Vec<String> {
    ui.meaning()
        .into_iter()
        .map(|meaning| meaning.name)
        .collect()
}

/// Builds `panel` over `world` in `ui`, and lays it out.
fn show(ui: &mut Ui, root: ElementId, panel: &mut WorldPanel, world: &World) {
    panel.build(ui, root, world);
    ui.frame();
}

#[test]
fn a_placement_shows_its_identity_and_its_prefab() {
    use slime_assets::AssetId;
    let (yard, lamp) = (AssetId::from_u128(1), AssetId::from_u128(2));
    let mut scenes = slime_world::SceneLibrary::new();
    scenes.insert_file(
        lamp,
        "lamp",
        "[entities.post.components]\nName = \"post\"\n",
    );
    scenes.insert_file(
        yard,
        "yard",
        "[assets]\nlamp = { id = \"00000000-0000-0000-0000-000000000002\" }\n\n\
         [entities.porch]\nprefab = \"lamp\"\n",
    );
    let mut world = World::new();
    let scene = scenes.resolve(world.registry(), yard).expect("resolved");
    world.spawn_scene("yard", &scene).expect("spawned");
    let porch = world
        .find(&slime_world::PersistentId::new(["yard", "porch"]))
        .expect("the placement");
    let (mut ui, root) = interface();
    let mut panel = WorldPanel::default();
    panel.select(porch);
    show(&mut ui, root, &mut panel, &world);
    let names = names(&ui);
    for shown in [
        "Identity: yard/porch",
        "Places the prefab `lamp`, 00000000-0000-0000-0000-000000000002",
    ] {
        assert!(names.iter().any(|name| name == shown), "{shown}: {names:?}");
    }
}

#[test]
fn a_despawned_entity_shows_nothing_and_stays_selected() {
    let (mut world, entities) = world();
    let (mut ui, root) = interface();
    let mut panel = WorldPanel::default();
    panel.select(entities[0]);
    world.despawn(entities[0]);
    show(&mut ui, root, &mut panel, &world);
    assert_eq!(panel.selected(), Some(entities[0]), "kept, as a handle");
    let names = names(&ui);
    assert!(
        names
            .iter()
            .any(|name| name == "Select an entity to see its components.")
    );
}

#[test]
fn the_entity_selected_is_found_again_by_its_identity_in_a_world_that_replaces_it() {
    let lamp = Light::Point {
        color: LinearColor::WHITE,
        intensity: 30.0,
        range: None,
    };
    let mut world = World::new();
    let first = world.spawn_keyed("lamp").expect("a key");
    world.insert(first, lamp);
    let (mut ui, root) = interface();
    let mut panel = WorldPanel::default();
    panel.select(first);
    show(&mut ui, root, &mut panel, &world);
    // Another world, as a load makes, where the handle names another entity.
    let mut replaced = World::new();
    let post = replaced.spawn_keyed("post").expect("a key");
    let found = replaced.spawn_keyed("lamp").expect("a key");
    replaced.insert(found, lamp);
    assert_eq!(post, first, "the handle names the post there");
    show(&mut ui, root, &mut panel, &replaced);
    assert_eq!(panel.selected(), Some(found));
    assert!(names(&ui).iter().any(|name| name == "Identity: lamp"));
    replaced.despawn(found);
    show(&mut ui, root, &mut panel, &replaced);
    assert_eq!(
        panel.selected(),
        Some(found),
        "kept while no entity has the identity"
    );
}
